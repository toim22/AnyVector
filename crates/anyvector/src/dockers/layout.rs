//! How the dockers are arranged (#144): the groups of dockers that share a tab strip, the side
//! of the drawing window each group is docked on (groups on one side stack top to bottom, each
//! taking a share of its height), and groups floating in their own window.
//!
//! The layout is kept with the settings and with each workspace. It reads leniently: a docker
//! this version doesn't know, or a value of the wrong type, is skipped rather than making the
//! settings unreadable.

use super::{DockerId, ALL};
use serde::Serialize;
use serde_json::Value;

/// A side of the drawing window.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default, Serialize, serde::Deserialize)]
pub enum DockerSide {
    Left,
    #[default]
    Right,
}

impl DockerSide {
    pub const BOTH: [DockerSide; 2] = [DockerSide::Left, DockerSide::Right];

    pub fn index(self) -> usize {
        match self {
            DockerSide::Left => 0,
            DockerSide::Right => 1,
        }
    }
}

/// The smallest share of a side's height a stacked group keeps.
pub const MIN_SHARE: f32 = 0.08;
/// The widths a docker side can take, in points.
pub const MIN_WIDTH: f32 = 220.0;
pub const MAX_WIDTH: f32 = 600.0;
pub const DEFAULT_WIDTH: f32 = 280.0;
/// The smallest floating docker window.
pub const MIN_FLOAT: [f32; 2] = [220.0, 160.0];

/// Dockers sharing one tab strip, one of them shown at a time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DockerGroup {
    /// The side it is docked on; for a floating group, the side it docks back to.
    pub side: DockerSide,
    pub floating: bool,
    /// The dockers, in tab order.
    pub dockers: Vec<DockerId>,
    /// The docker shown.
    pub active: Option<DockerId>,
    /// The share of the side's height it takes when other groups are stacked with it (the
    /// shares of a side add up to 1).
    pub share: f32,
    /// A floating group's window: left, top, width and height, in points.
    pub rect: [f32; 4],
}

impl DockerGroup {
    pub fn docked(side: DockerSide, dockers: Vec<DockerId>) -> DockerGroup {
        let active = dockers.last().copied();
        DockerGroup { side, floating: false, dockers, active, share: 1.0, rect: [0.0; 4] }
    }

    pub fn float(home: DockerSide, dockers: Vec<DockerId>, rect: [f32; 4]) -> DockerGroup {
        let active = dockers.last().copied();
        DockerGroup { side: home, floating: true, dockers, active, share: 1.0, rect }
    }

    /// Docked on `side` (not floating).
    pub fn on(&self, side: DockerSide) -> bool {
        !self.floating && self.side == side
    }
}

/// The whole arrangement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DockerLayout {
    /// The groups: a side shows its docked groups top to bottom in this order, floating windows
    /// are drawn in this order (the last on top).
    pub groups: Vec<DockerGroup>,
    /// The width of each side's docker area, in points.
    pub left_width: f32,
    pub right_width: f32,
}

impl Default for DockerLayout {
    fn default() -> DockerLayout {
        DockerLayout { groups: vec![], left_width: DEFAULT_WIDTH, right_width: DEFAULT_WIDTH }
    }
}

/// Where a docker (or a group) goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Into a group's tabs, at this place.
    Join { group: usize, at: usize },
    /// A new group stacked above (`after` false) or below a docked group.
    Stack { group: usize, after: bool },
    /// A new group at the bottom of a side.
    Side(DockerSide),
    /// A floating window at this rectangle (left, top, width, height).
    Float([f32; 4]),
}

fn docker_named(n: &str) -> Option<DockerId> {
    ALL.iter().copied().find(|d| format!("{d:?}") == n)
}

/// The dockers named in a settings list (`open_dockers`), unknown names left out.
pub fn dockers_named(names: &[String]) -> Vec<DockerId> {
    let mut out = vec![];
    for d in names.iter().filter_map(|n| docker_named(n)) {
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// The names kept in the settings list (`open_dockers`).
pub fn names_of(dockers: impl IntoIterator<Item = DockerId>) -> Vec<String> {
    dockers.into_iter().map(|d| format!("{d:?}")).collect()
}

/// The arrangement settings (or a workspace) hold: `layout` itself, or for those saved before
/// 0.6.1, which have only the list of open dockers and the docker width, one group on the right.
pub fn effective(layout: &DockerLayout, open_dockers: &[String], docker_width: f32) -> DockerLayout {
    if layout.is_empty() {
        DockerLayout::one_group(dockers_named(open_dockers), docker_width)
    } else {
        let mut l = layout.clone();
        l.normalize();
        l
    }
}

fn num(v: Option<&Value>, default: f32) -> f32 {
    v.and_then(Value::as_f64).map(|x| x as f32).filter(|x| x.is_finite()).unwrap_or(default)
}

impl DockerGroup {
    fn from_json(v: &Value) -> Option<DockerGroup> {
        let side = match v.get("side").and_then(Value::as_str) {
            Some("Left") => DockerSide::Left,
            _ => DockerSide::Right,
        };
        let dockers: Vec<DockerId> = v.get("dockers").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).filter_map(docker_named).collect()).unwrap_or_default();
        let active = v.get("active").and_then(Value::as_str).and_then(docker_named);
        let mut rect = [0.0; 4];
        if let Some(a) = v.get("rect").and_then(Value::as_array) {
            for (i, x) in a.iter().take(4).enumerate() {
                rect[i] = num(Some(x), 0.0);
            }
        }
        let floating = v.get("floating").and_then(Value::as_bool).unwrap_or(false);
        Some(DockerGroup { side, floating, dockers, active, share: num(v.get("share"), 1.0), rect })
    }
}

impl DockerLayout {
    /// Reads a layout, skipping what it doesn't understand.
    pub fn from_json(v: &Value) -> DockerLayout {
        let d = DockerLayout::default();
        let groups = v.get("groups").and_then(Value::as_array).map(|a| a.iter().filter_map(DockerGroup::from_json).collect()).unwrap_or_default();
        let mut l = DockerLayout { groups, left_width: num(v.get("left_width"), d.left_width), right_width: num(v.get("right_width"), d.right_width) };
        l.normalize();
        l
    }

    /// The 0.6.0 arrangement: every docker in one group on the right.
    pub fn one_group(dockers: Vec<DockerId>, right_width: f32) -> DockerLayout {
        let mut l = DockerLayout { right_width, ..Default::default() };
        if !dockers.is_empty() {
            l.groups.push(DockerGroup::docked(DockerSide::Right, dockers));
        }
        l.normalize();
        l
    }

    /// Every docker of the layout: group by group, in tab order.
    pub fn dockers(&self) -> impl Iterator<Item = DockerId> + '_ {
        self.groups.iter().flat_map(|g| g.dockers.iter().copied())
    }

    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.dockers.is_empty())
    }

    pub fn contains(&self, d: DockerId) -> bool {
        self.group_of(d).is_some()
    }

    /// The group a docker is in.
    pub fn group_of(&self, d: DockerId) -> Option<usize> {
        self.groups.iter().position(|g| g.dockers.contains(&d))
    }

    /// The docked groups of a side, top to bottom.
    pub fn side_groups(&self, side: DockerSide) -> Vec<usize> {
        (0..self.groups.len()).filter(|i| self.groups[*i].on(side)).collect()
    }

    pub fn width(&self, side: DockerSide) -> f32 {
        match side {
            DockerSide::Left => self.left_width,
            DockerSide::Right => self.right_width,
        }
    }

    pub fn width_mut(&mut self, side: DockerSide) -> &mut f32 {
        match side {
            DockerSide::Left => &mut self.left_width,
            DockerSide::Right => &mut self.right_width,
        }
    }

    /// Takes a docker out of its group (an emptied group stays until [`Self::normalize`]); gives
    /// the group and the place it had.
    pub fn take(&mut self, d: DockerId) -> Option<(usize, usize)> {
        let g = self.group_of(d)?;
        let grp = &mut self.groups[g];
        let i = grp.dockers.iter().position(|x| *x == d)?;
        grp.dockers.remove(i);
        if grp.active == Some(d) {
            // the tab next to it comes forward
            grp.active = grp.dockers.get(i.min(grp.dockers.len().saturating_sub(1))).copied();
        }
        Some((g, i))
    }

    /// A share for a new group at the bottom of a side: as much as each group there has now.
    fn new_share(&self, side: DockerSide) -> f32 {
        let n = self.side_groups(side).len();
        if n == 0 {
            1.0
        } else {
            1.0 / n as f32
        }
    }

    /// Puts dockers (one, or a whole group's in tab order) where `t` says, the first of them
    /// shown. Where they were, they are taken out.
    pub fn place(&mut self, dockers: &[DockerId], t: Target) {
        let Some(&first) = dockers.first() else { return };
        // where they were (adjusts a place in the same group)
        let mut taken = vec![];
        for d in dockers {
            if let Some(p) = self.take(*d) {
                taken.push(p);
            }
        }
        match t {
            Target::Join { group, at } if group < self.groups.len() => {
                let mut at = at;
                for (g, i) in &taken {
                    if *g == group && *i < at {
                        at -= 1;
                    }
                }
                let grp = &mut self.groups[group];
                let at = at.min(grp.dockers.len());
                for (k, d) in dockers.iter().enumerate() {
                    grp.dockers.insert(at + k, *d);
                }
                grp.active = Some(first);
            }
            Target::Stack { group, after } if group < self.groups.len() && !self.groups[group].floating => {
                let side = self.groups[group].side;
                // the group stacked on halves its share with the new one
                let share = self.groups[group].share.max(MIN_SHARE * 2.0) / 2.0;
                self.groups[group].share = share;
                let mut g = DockerGroup::docked(side, dockers.to_vec());
                g.share = share;
                g.active = Some(first);
                self.groups.insert(if after { group + 1 } else { group }, g);
            }
            Target::Side(side) => {
                let mut g = DockerGroup::docked(side, dockers.to_vec());
                g.share = self.new_share(side);
                g.active = Some(first);
                // after the side's last group, so it shows at the bottom
                let at = self.side_groups(side).last().map(|i| i + 1).unwrap_or(self.groups.len());
                self.groups.insert(at, g);
            }
            Target::Float(rect) => {
                // it docks back to the side it came from
                let home = taken.first().and_then(|(g, _)| self.groups.get(*g)).map(|g| g.side).unwrap_or_default();
                let mut g = DockerGroup::float(home, dockers.to_vec(), rect);
                g.active = Some(first);
                self.groups.push(g);
            }
            // a group that is gone: at the bottom of the right side
            _ => return self.place(dockers, Target::Side(DockerSide::Right)),
        }
        self.normalize();
    }

    /// Moves a whole group where `t` says (the docker it shows stays in front).
    pub fn place_group(&mut self, group: usize, t: Target) {
        let Some(g) = self.groups.get(group) else { return };
        let (dockers, active) = (g.dockers.clone(), g.active);
        if let Target::Float(rect) = t {
            // a floating group just moves; a docked one floats as a whole
            let g = &mut self.groups[group];
            g.floating = true;
            g.rect = rect;
            self.normalize();
            return;
        }
        self.place(&dockers, t);
        if let (Some(a), Some(g)) = (active, active.and_then(|a| self.group_of(a))) {
            self.groups[g].active = Some(a);
        }
    }

    /// Docks a docker on a side: into the side's last group, or a new group when the side has
    /// none. A docker already docked there stays where it is.
    pub fn dock(&mut self, d: DockerId, side: DockerSide) {
        if self.group_of(d).is_some_and(|g| self.groups[g].on(side)) {
            return;
        }
        match self.side_groups(side).last() {
            Some(&g) => {
                let at = self.groups[g].dockers.len();
                self.place(&[d], Target::Join { group: g, at });
            }
            None => self.place(&[d], Target::Side(side)),
        }
    }

    /// Docks a whole group on a side, as [`Self::dock`] does one docker.
    pub fn dock_group(&mut self, group: usize, side: DockerSide) {
        let Some(g) = self.groups.get(group) else { return };
        if g.on(side) {
            return;
        }
        match self.side_groups(side).last() {
            Some(&to) => {
                let at = self.groups[to].dockers.len();
                self.place_group(group, Target::Join { group: to, at });
            }
            None => self.place_group(group, Target::Side(side)),
        }
    }

    /// The 0.6.0 arrangement again: every docker in one group on the right, in the order they
    /// are now, the one shown kept in front.
    pub fn reset(&mut self, active: Option<DockerId>) {
        let dockers: Vec<DockerId> = self.dockers().collect();
        let width = self.right_width;
        *self = DockerLayout::one_group(dockers, width);
        if let (Some(g), Some(a)) = (self.groups.first_mut(), active) {
            if g.dockers.contains(&a) {
                g.active = Some(a);
            }
        }
    }

    /// Keeps the layout whole: each docker in one group only, no empty groups, a shown docker
    /// in every group, the shares of each side adding up to 1, sizes in range.
    pub fn normalize(&mut self) {
        let mut seen = vec![];
        for g in self.groups.iter_mut() {
            g.dockers.retain(|d| {
                let dup = seen.contains(d);
                seen.push(*d);
                !dup
            });
        }
        self.groups.retain(|g| !g.dockers.is_empty());
        for g in self.groups.iter_mut() {
            if !g.active.is_some_and(|a| g.dockers.contains(&a)) {
                g.active = g.dockers.last().copied();
            }
            if !g.share.is_finite() || g.share <= 0.0 {
                g.share = 1.0;
            }
            if g.floating {
                g.rect[2] = if g.rect[2].is_finite() { g.rect[2].max(MIN_FLOAT[0]) } else { 300.0 };
                g.rect[3] = if g.rect[3].is_finite() { g.rect[3].max(MIN_FLOAT[1]) } else { 420.0 };
                for v in &mut g.rect[..2] {
                    if !v.is_finite() {
                        *v = 200.0;
                    }
                }
            } else {
                g.rect = [0.0; 4];
            }
        }
        for side in DockerSide::BOTH {
            let gs = self.side_groups(side);
            let total: f32 = gs.iter().map(|g| self.groups[*g].share).sum();
            for g in &gs {
                let s = &mut self.groups[*g].share;
                *s = (*s / total).max(MIN_SHARE);
            }
            // the floor can push the sum over 1: take it back from the others
            let total: f32 = gs.iter().map(|g| self.groups[*g].share).sum();
            for g in &gs {
                self.groups[*g].share /= total;
            }
        }
        for w in [&mut self.left_width, &mut self.right_width] {
            *w = if w.is_finite() { w.clamp(MIN_WIDTH, MAX_WIDTH) } else { DEFAULT_WIDTH };
        }
    }

    /// Only the dockers `keep` allows (a workspace element import).
    pub fn retain(&mut self, keep: impl Fn(DockerId) -> bool) {
        for g in self.groups.iter_mut() {
            g.dockers.retain(|d| keep(*d));
        }
        self.normalize();
    }
}

impl<'de> serde::Deserialize<'de> for DockerLayout {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<DockerLayout, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(DockerLayout::from_json(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::DockerId::*;
    use super::*;

    fn layout() -> DockerLayout {
        DockerLayout::one_group(vec![Properties, Objects, Pages], 300.0)
    }

    #[test]
    fn one_group_from_the_open_dockers() {
        let l = DockerLayout::one_group(dockers_named(&["Properties".into(), "Nope".into(), "Objects".into(), "Properties".into()]), 320.0);
        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.groups[0].dockers, vec![Properties, Objects]);
        assert_eq!(l.groups[0].active, Some(Objects));
        assert!(l.groups[0].on(DockerSide::Right));
        assert_eq!(l.right_width, 320.0);
        assert!(DockerLayout::one_group(vec![], 280.0).groups.is_empty());
    }

    #[test]
    fn json_round_trip_and_lenient_reading() {
        let mut l = layout();
        l.place(&[Pages], Target::Side(DockerSide::Left));
        l.place(&[Objects], Target::Float([10.0, 20.0, 300.0, 400.0]));
        l.left_width = 250.0;
        let s = serde_json::to_string(&l).unwrap();
        let back: DockerLayout = serde_json::from_str(&s).unwrap();
        assert_eq!(back, l);
        // names, not numbers: a docker this version doesn't know is skipped, a bad value too
        assert!(s.contains("\"Pages\""), "{s}");
        let odd = r#"{"groups":[{"side":"Left","dockers":["Pages","FutureDocker"],"share":"x"},{"side":"Up","floating":true,"dockers":["Objects"],"rect":[1,2]}],"left_width":"wide"}"#;
        let l: DockerLayout = serde_json::from_str(odd).unwrap();
        assert_eq!(l.groups.len(), 2);
        assert_eq!(l.groups[0].dockers, vec![Pages]);
        assert_eq!(l.groups[0].share, 1.0);
        assert!(l.groups[1].floating && l.groups[1].side == DockerSide::Right);
        assert_eq!(l.groups[1].rect[2..], [MIN_FLOAT[0], MIN_FLOAT[1]]);
        assert_eq!(l.left_width, DEFAULT_WIDTH);
        // not even an object
        let l: DockerLayout = serde_json::from_str("[1, 2]").unwrap();
        assert!(l.groups.is_empty());
    }

    #[test]
    fn reorder_within_a_group() {
        let mut l = layout();
        // Pages before Properties
        l.place(&[Pages], Target::Join { group: 0, at: 0 });
        assert_eq!(l.groups[0].dockers, vec![Pages, Properties, Objects]);
        assert_eq!(l.groups[0].active, Some(Pages));
        // Pages to the end (after Objects): the place counts before it was taken out
        l.place(&[Pages], Target::Join { group: 0, at: 3 });
        assert_eq!(l.groups[0].dockers, vec![Properties, Objects, Pages]);
        l.place(&[Properties], Target::Join { group: 0, at: 2 });
        assert_eq!(l.groups[0].dockers, vec![Objects, Properties, Pages]);
    }

    #[test]
    fn dock_on_the_other_side_and_back() {
        let mut l = layout();
        l.dock(Objects, DockerSide::Left);
        assert_eq!(l.side_groups(DockerSide::Left).len(), 1);
        assert_eq!(l.groups[l.group_of(Objects).unwrap()].side, DockerSide::Left);
        assert_eq!(l.groups[l.group_of(Properties).unwrap()].dockers, vec![Properties, Pages]);
        // a second one joins the left group
        l.dock(Pages, DockerSide::Left);
        assert_eq!(l.groups[l.group_of(Pages).unwrap()].dockers, vec![Objects, Pages]);
        // docking where it is does nothing
        let before = l.clone();
        l.dock(Properties, DockerSide::Right);
        assert_eq!(l, before);
        // the last docker of a side leaving takes the side away
        l.dock(Objects, DockerSide::Right);
        l.dock(Pages, DockerSide::Right);
        assert!(l.side_groups(DockerSide::Left).is_empty());
        assert_eq!(l.groups.len(), 1);
    }

    #[test]
    fn stack_and_split_shares() {
        let mut l = layout();
        l.place(&[Pages], Target::Stack { group: 0, after: true });
        let gs = l.side_groups(DockerSide::Right);
        assert_eq!(gs.len(), 2);
        assert_eq!(l.groups[gs[0]].dockers, vec![Properties, Objects]);
        assert_eq!(l.groups[gs[1]].dockers, vec![Pages]);
        assert!((l.groups[gs[0]].share - 0.5).abs() < 1e-4 && (l.groups[gs[1]].share - 0.5).abs() < 1e-4);
        // above the first one: a third of the side each? the first halves its own share
        l.place(&[Objects], Target::Stack { group: 0, after: false });
        let gs = l.side_groups(DockerSide::Right);
        assert_eq!(gs.len(), 3);
        assert_eq!(l.groups[gs[0]].dockers, vec![Objects]);
        let sum: f32 = gs.iter().map(|g| l.groups[*g].share).sum();
        assert!((sum - 1.0).abs() < 1e-4);
        // a group closing gives its share to the others
        l.take(Objects);
        l.normalize();
        let gs = l.side_groups(DockerSide::Right);
        assert_eq!(gs.len(), 2);
        let sum: f32 = gs.iter().map(|g| l.groups[*g].share).sum();
        assert!((sum - 1.0).abs() < 1e-4);
        // a new group at the bottom of a side takes an equal share
        l.place(&[Objects], Target::Side(DockerSide::Right));
        let gs = l.side_groups(DockerSide::Right);
        assert_eq!(l.groups[*gs.last().unwrap()].dockers, vec![Objects]);
        assert!(gs.iter().all(|g| l.groups[*g].share > 0.2));
    }

    #[test]
    fn float_and_dock_back_home() {
        let mut l = layout();
        l.place(&[Pages], Target::Side(DockerSide::Left));
        l.place(&[Pages], Target::Float([100.0, 100.0, 50.0, 50.0]));
        let g = l.group_of(Pages).unwrap();
        assert!(l.groups[g].floating);
        assert_eq!(l.groups[g].side, DockerSide::Left, "it remembers where it came from");
        assert_eq!(l.groups[g].rect, [100.0, 100.0, MIN_FLOAT[0], MIN_FLOAT[1]]);
        assert!(l.side_groups(DockerSide::Left).is_empty());
        // another docker joins the floating window
        l.place(&[Objects], Target::Join { group: g, at: 0 });
        assert_eq!(l.groups[l.group_of(Objects).unwrap()].dockers, vec![Objects, Pages]);
        // the whole window moves, then docks on the right with its dockers
        let g = l.group_of(Pages).unwrap();
        l.place_group(g, Target::Float([5.0, 6.0, 400.0, 300.0]));
        assert_eq!(l.groups[l.group_of(Pages).unwrap()].rect, [5.0, 6.0, 400.0, 300.0]);
        let g = l.group_of(Pages).unwrap();
        l.groups[g].active = Some(Pages);
        l.dock_group(g, DockerSide::Right);
        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.groups[0].dockers, vec![Properties, Objects, Pages]);
        assert_eq!(l.groups[0].active, Some(Pages));
        assert!(!l.groups[0].floating);
    }

    #[test]
    fn a_docked_group_floats_whole() {
        let mut l = layout();
        l.place(&[Pages], Target::Stack { group: 0, after: true });
        let g = l.group_of(Properties).unwrap();
        l.place_group(g, Target::Float([0.0, 0.0, 300.0, 300.0]));
        let g = l.group_of(Properties).unwrap();
        assert!(l.groups[g].floating);
        assert_eq!(l.groups[g].dockers, vec![Properties, Objects]);
        assert_eq!(l.side_groups(DockerSide::Right).len(), 1);
        assert_eq!(l.groups[l.side_groups(DockerSide::Right)[0]].share, 1.0);
    }

    #[test]
    fn reset_and_retain() {
        let mut l = layout();
        l.dock(Objects, DockerSide::Left);
        l.place(&[Pages], Target::Float([0.0, 0.0, 300.0, 300.0]));
        l.reset(Some(Objects));
        assert_eq!(l.groups.len(), 1);
        assert_eq!(l.groups[0].dockers, vec![Properties, Objects, Pages]);
        assert_eq!(l.groups[0].active, Some(Objects));
        assert_eq!(l.right_width, 300.0);
        l.retain(|d| d != Objects);
        assert_eq!(l.groups[0].dockers, vec![Properties, Pages]);
        assert_eq!(l.groups[0].active, Some(Pages));
        l.retain(|_| false);
        assert!(l.groups.is_empty() && l.is_empty());
    }

    #[test]
    fn normalize_mends_a_broken_layout() {
        let mut l = DockerLayout {
            groups: vec![
                DockerGroup { side: DockerSide::Right, floating: false, dockers: vec![Pages, Objects], active: Some(Color), share: f32::NAN, rect: [1.0; 4] },
                DockerGroup { side: DockerSide::Right, floating: false, dockers: vec![Objects], active: None, share: 3.0, rect: [0.0; 4] },
                DockerGroup { side: DockerSide::Right, floating: false, dockers: vec![], active: None, share: 1.0, rect: [0.0; 4] },
            ],
            left_width: 10.0,
            right_width: 9000.0,
        };
        l.normalize();
        assert_eq!(l.groups.len(), 1, "a docker shows once, and empty groups go");
        assert_eq!(l.groups[0].active, Some(Objects));
        assert_eq!(l.groups[0].share, 1.0);
        assert_eq!(l.groups[0].rect, [0.0; 4]);
        assert_eq!((l.left_width, l.right_width), (MIN_WIDTH, MAX_WIDTH));
    }
}
