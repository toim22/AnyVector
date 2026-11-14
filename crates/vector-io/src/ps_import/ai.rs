//! Adobe Illustrator's operators, for Illustrator files (AI 3 to 8 and their EPS) that use
//! them without including the procsets that define them. They are looked up only when the
//! file has not defined the name itself.
//!
//! Illustrator keeps separate fill and stroke colors and paints with `f`, `S`, `B` and their
//! closing forms; compound paths (`*u` … `*U`) are painted once, as one curve.

use super::interp::Interp;
use super::obj::*;

/// The operators, written in PostScript.
pub const PROCSET: &str = r#"
/_fc {0 setgray} def /_sc {0 setgray} def
/_eo false def /_cp false def /_pm 0 def /_clp false def
/_doF { gsave _fc _eo {eofill} {fill} ifelse grestore } bind def
/_doS { gsave _sc stroke grestore } bind def
/_paint {
  _cp { _pm or /_pm exch def }
      { dup 1 and 0 ne {_doF} if 2 and 0 ne {_doS} if
        _clp { clip /_clp false def } if newpath } ifelse
} bind def
/m {moveto} bind def /l {lineto} bind def /L {lineto} bind def
/c {curveto} bind def /C {curveto} bind def
/v { currentpoint 6 2 roll curveto } bind def /V /v load def
/y { 2 copy curveto } bind def /Y /y load def
/h {closepath} bind def /H {} def
/N { 0 _paint } bind def /n { closepath 0 _paint } bind def
/F { 1 _paint } bind def /f { closepath 1 _paint } bind def
/S { 2 _paint } bind def /s { closepath 2 _paint } bind def
/B { 3 _paint } bind def /b { closepath 3 _paint } bind def
/*u { /_cp true def /_pm 0 def } bind def
/*U { /_cp false def
     _pm 1 and 0 ne {_doF} if _pm 2 and 0 ne {_doS} if
     _clp { clip /_clp false def } if newpath } bind def
/W { /_clp true def } bind def
/g { /_fv exch def /_fc { _fv setgray } def } bind def
/G { /_sv exch def /_sc { _sv setgray } def } bind def
/k { 4 array astore /_fv exch def /_fc { _fv aload pop setcmykcolor } def } bind def
/K { 4 array astore /_sv exch def /_sc { _sv aload pop setcmykcolor } def } bind def
/Xa { 3 array astore /_fv exch def /_fc { _fv aload pop setrgbcolor } def } bind def
/XA { 3 array astore /_sv exch def /_sc { _sv aload pop setrgbcolor } def } bind def
/x { /_ft exch def /_fn exch def 4 array astore /_fv exch def
     /_fc { _fv aload pop _fn findcmykcustomcolor 1 _ft sub setcustomcolor } def } bind def
/X { /_st exch def /_sn exch def 4 array astore /_sv exch def
     /_sc { _sv aload pop _sn findcmykcustomcolor 1 _st sub setcustomcolor } def } bind def
/Xx { 0 eq { x } { /_ft exch def /_fn exch def 3 array astore /_fv exch def
     /_fc { _fv aload pop _fn findrgbcustomcolor 1 _ft sub setcustomcolor } def } ifelse } bind def
/XX { 0 eq { X } { /_st exch def /_sn exch def 3 array astore /_sv exch def
     /_sc { _sv aload pop _sn findrgbcustomcolor 1 _st sub setcustomcolor } def } ifelse } bind def
/w {setlinewidth} bind def /j {setlinejoin} bind def /J {setlinecap} bind def
/M {setmiterlimit} bind def /d {setdash} bind def
/i {pop} def /D {pop} def /A {pop} def /O {pop} def /R {pop} def
/XR { 0 ne /_eo exch def } bind def
/u {} def /U {} def /q {gsave} bind def /Q {grestore} bind def
/Ln {pop} def /Lb { 10 {pop} repeat } def /LB {} def
/Ap {pop} def /Ar {pop} def /XT {pop pop} def
/To { pop /_tx 0 def /_ty 0 def /_tm matrix def } bind def /TO {} def
/Tp { pop 6 array astore /_tm exch def /_tx 0 def /_ty 0 def } bind def /TP {} def
/Tm { 6 array astore /_tm exch def /_tx 0 def /_ty 0 def } bind def
/Td { /_dty exch def /_dtx exch def /_tx _tx _dtx add def /_ty _ty _dty add def } bind def
/_tl 0 def /T* { /_ty _ty _tl sub def } bind def
/Tl { pop /_tl exch def } bind def
/_tfn /Helvetica def /_tsz 12 def
/Tf { /_tsz exch def /_tfn exch def } bind def
/_tshow { gsave _tm concat _tx _ty moveto _tfn findfont _tsz scalefont setfont _fc
          dup show currentpoint /_ty exch def /_tx exch def pop grestore } bind def
/Tx { _tshow } bind def /Tj { _tshow } bind def /TX {pop} def
/Tr {pop} def /Ta {pop} def /Tq {pop} def /Tc {pop} def /Tw {pop} def /Ts {pop} def
/Tz {pop} def /Tt {pop} def /TA {pop} def /Tv {pop} def /Ti {pop pop pop} def
/Tk {pop pop} def /TK {pop pop} def /TC {pop pop pop} def /TW {pop pop pop} def
/T+ {} def /T- {} def /Tb {} def /TB {} def /Tg {pop} def
/TZ { pop pop pop /_tzb exch def /_tzn exch def cleartomark _tzn _tzb findfont definefont pop } bind def
/Bb {} def /BB {pop} def /Bh {pop pop pop pop} def /Bg { 10 {pop} repeat } def
/Xm {pop pop pop pop pop pop} def
"#;

/// A dictionary standing in for a procset the file names but doesn't include.
pub fn stub_procset(i: &mut Interp) -> DictRef {
    let d = new_dict(8);
    i.dput(&d, "initialize", Obj::proc_(vec![]));
    i.dput(&d, "terminate", Obj::proc_(vec![]));
    d
}
