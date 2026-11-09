"""Writes crates/anyvector/src/fxdata.rs: the page of every bitmap effect of the Effects menu, as
the Properties docker shows it (Bitmap Effects).

    python xtask/fxgen.py

The labels and the option lists come from the CorelDRAW 2026 user-interface definition the
reference was captured from (`<CorelDRAW>\\Draw\\UIConfig\\DrawUI.items.xml` and
`DrawUI.containers.xml`, with the captions of `Resources\\caption.csv` and
`Languages\\EN\\Data\\strings.xml`; the folder defaults to
`C:\\Program Files\\Corel\\CorelDRAW Graphics Suite\\27` and can be changed with ANYVECTOR_COREL).
The order of the controls, their ranges and their default values are those of the reference
screenshots (`03-telas\\bitmap\\06 Effects`), written in PAGES below.
"""
import os
import re
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
COREL = os.environ.get('ANYVECTOR_COREL', r'C:\Program Files\Corel\CorelDRAW Graphics Suite\27')
UI = os.path.join(COREL, 'Draw', 'UIConfig')

# ------------------------------------------------------------------------------ the definition

strings = {}
for m in re.finditer(r'<string guid="([^"]+)">(.*?)</string>', open(os.path.join(COREL, 'Languages', 'EN', 'Data', 'strings.xml'), encoding='utf-8').read(), re.S):
    strings[m.group(1).lower()] = m.group(2)


def load_csv(name):
    d = {}
    for line in open(os.path.join(COREL, 'Resources', name), encoding='utf-8'):
        p = line.strip().split(',')
        if len(p) >= 2:
            d[p[0].lower()] = strings.get(p[1].lower(), '')
    return d


cap = load_csv('caption.csv')
items = {}
for el in ET.parse(os.path.join(UI, 'DrawUI.items.xml')).getroot().iter('itemData'):
    if el.get('guid'):
        items[el.get('guid').lower()] = el
conts = {}
for el in ET.parse(os.path.join(UI, 'DrawUI.containers.xml')).getroot().iter():
    if el.tag == 'container' and el.get('guid'):
        conts[el.get('guid').lower()] = el


def caption(g, depth=0):
    it = items.get(g)
    c = cap.get(g, '')
    if not c and it is not None:
        cr = it.get('captionRef')
        if cr:
            c = cap.get(cr.lower(), '') or strings.get(cr.lower(), '')
        if not c and it.get('caption') and not it.get('caption').startswith('*'):
            c = it.get('caption')
        if not c:
            for a in ('text', 'caption'):
                m = re.search(r"\*CT\('([^']+)'\)", it.get(a) or '')
                if m:
                    k = m.group(1).lower()
                    c = cap.get(k, '') or strings.get(k, '')
                    if not c and k in items and depth < 4:
                        c = caption(k, depth + 1)
                    break
    return c.replace('&amp;', '&').replace('&&', '\x01').replace('&', '').replace('\x01', '&').strip()


def leaves(el, depth=0):
    out = []
    for ch in el:
        if ch.tag == 'item' and ch.get('guidRef'):
            out.append(ch.get('guidRef').lower())
        else:
            ref = ch.get('guidRef')
            if ref and depth < 8 and ref.lower() in conts:
                out += leaves(conts[ref.lower()], depth + 1)
            out += leaves(ch, depth + 1)
    return out


def path_of(it, attr):
    m = re.search(r'Path=(\w+)', it.get(attr) or '')
    return m.group(1) if m else None


def ds_items(ds):
    return {g for g, it in items.items() if any(('DataSource=' + ds) in v for v in it.attrib.values())}


def page_container(ds):
    mine = ds_items(ds)
    best = []
    for cg, el in conts.items():
        lv = leaves(el)
        n = len([x for x in lv if x in mine])
        if n:
            best.append((n, any(x.tag == 'gridContainer' for x in el.iter()), len(lv), cg))
    if not best:
        return None
    mx = max(b[0] for b in best)
    return sorted([b for b in best if b[0] == mx], key=lambda b: (b[1], b[2]))[0][3]


def spec(ds):
    """Label and options of each setting of an effect page, by setting name."""
    out = {}
    cg = page_container(ds)
    if not cg:
        return out
    label = None
    for g in leaves(conts[cg]):
        it = items.get(g)
        if it is None:
            continue
        t = it.get('type', '')
        c = caption(g)
        if t == 'statusText':
            label = c
            continue
        p = None
        for a in ('value', 'check', 'currentActiveOfGroup', 'currentItem', 'selectedIndex'):
            p = p or path_of(it, a)
        if not p:
            continue
        d = out.setdefault(p, {'label': None, 'opts': []})
        if t == 'groupedRadioButton':
            if d['label'] is None:
                d['label'] = label or ''
            d['opts'].append(c)
        elif t == 'checkButton':
            d['label'] = d['label'] or c
        elif d['label'] is None:
            d['label'] = label or c
        if t != 'groupedRadioButton':
            label = None
        elif d['opts'] and len(d['opts']) == 1:
            label = None
    return out


def effect_guids():
    """The Effects menu item of each effect (its On<Name> command)."""
    data = open(os.path.join(ROOT, 'crates', 'anyvector', 'src', 'ui', 'data.rs'), encoding='utf-8').read()
    found = {}
    for g, it in items.items():
        m = re.search(r'EffectsDialogFactoryDS;Path=On(\w+)', it.get('onInvoke') or '')
        if m and ('id: "%s"' % g) in data.lower():
            found.setdefault(m.group(1), (g, caption(g).replace('...', '')))
    return found

# ------------------------------------------------------------------------------ the pages


def C3(h):
    h = h.lstrip('#')
    return [int(h[i:i + 2], 16) for i in (0, 2, 4)]


def S(key, d, lo=0, hi=100, label=None, step=1, dec=0, log=False, g=None, unit='', spin=True):
    return dict(k='Slider', key=key, d=d, lo=lo, hi=hi, label=label, step=step, dec=dec, log=log, g=g, unit=unit, spin=spin)


def N(key, d, lo=0, hi=100, label=None, step=1, dec=0, unit=''):
    return dict(k='Spin', key=key, d=d, lo=lo, hi=hi, label=label, step=step, dec=dec, unit=unit)


def A(key, d, label=None, quarter=False):
    return dict(k='Angle', key=key, d=d, label=label, quarter=quarter)


def R(key, d=0, label=None, opts=None):
    return dict(k='Radio', key=key, d=d, label=label, opts=opts)


def I(key, icons, d=0, label=''):
    return dict(k='Icons', key=key, icons=icons, d=d, label=label)


def CK(key, d=False, label=None):
    return dict(k='Check', key=key, d=d, label=label)


def K(key, opts, d=0, label=None, inline=False):
    return dict(k='Combo', key=key, opts=opts, d=d, label=label, inline=inline)


def L(key, d=None, label=None, dropper=True):
    return dict(k='Color', key=key, d=d, label=label, dropper=dropper)


def T(key, swatch, d=False):
    return dict(k='Toggle', key=key, swatch=swatch, d=d)


def X(x=0.5, y=0.5):
    return dict(k='Center', x=x, y=y)


def LK(key='Lock', d=True):
    return dict(k='Lock', key=key, d=d)


def SD(key='Randomize', label='Randomize'):
    return dict(k='Seed', key=key, label=label)


def H(text):
    return dict(k='Heading', text=text)


def TH(ops):
    return dict(k='Thumbs', ops=ops)


def ST(key, opts, d=0):
    return dict(k='Styles', key=key, opts=opts, d=d)


def SW(key, opts, d=0, label='', split=0):
    return dict(k='Swatches', key=key, opts=opts, d=d, label=label, split=split)


def PR(opts=('Default',)):
    return dict(k='Presets', opts=list(opts))


def TX(key, label='Texture'):
    return dict(k='Texture', key=key, label=label)


def CU(key, hist=False):
    return dict(k='Curve', key=key, hist=hist)


def HI(markers=False):
    return dict(k='Histogram', markers=markers)


def WHEN(key, val, ctls):
    return dict(k='When', key=key, val=val, ctls=ctls)


def EN(key, ctls):
    return dict(k='Enable', key=key, ctls=ctls)


def TABS(key, tabs):
    return dict(k='Tabs', key=key, tabs=tabs)


def ROW(ctls):
    return dict(k='Row', ctls=ctls)


def U(kind):
    return dict(k=kind)


P = U('Preview')
WHITE, BLACK, GRAY = '#ffffff', '#000000', '#808080'
RAINBOW = 'hue'
CHANNELS = ['RGB', 'Red', 'Green', 'Blue']


def tri(c):
    return (BLACK, c, WHITE)


PAGES = {
    # 3D Effects
    'ThreeDRotate': [U('Rotate3d'), N('VertAngle', 0, -75, 75), N('HorizAngle', 0, -75, 75), CK('BestFit', True)],
    'Cylinder': [P, R('Mode', 0), S('Percentage', 25, -100, 100)],
    'Emboss': [S('Depth', 2, 1, 20), S('Level', 100, 1, 500, log=True), A('Direction', 45), R('ColorMode', 1), L('ColorItem', WHITE, label='')],
    'PageCurl': [I('CornerType', [('curl_nw', 'Top left'), ('curl_ne', 'Top right'), ('curl_sw', 'Bottom left'), ('curl_se', 'Bottom right')], 1), R('DirectionType', 0), R('PaperType', 1),
                 L('PaintColorItem', GRAY), L('BackColorItem', WHITE), S('Width', 50, 1, 100, unit='%'), S('Height', 50, 1, 100, unit='%')],
    'PinchPunch': [P, X(), S('PinchPunch', 100, -100, 100)],
    'Sphere': [P, X(), R('Optimize', 0), S('Percentage', 25, -100, 100)],
    'Zigzag': [P, X(), R('WaveType', 1, label='Type'), R('WaveOptimization', 0, label='Optimize'), S('Waves', 20, 1, 100), S('Strength', 20, -100, 100), EN('WaveType=0', [S('Damping', 0, 0, 100)])],
    # Adjust
    'Equalize': [K('Channel', CHANNELS, label=''), U('Levels')],
    'HistogramEqualize': [K('Channel', CHANNELS, label=''), CK('PreserveColor', False), HI(True), EN('Never', [ROW([N('LowerLimit', 0, label=''), N('UpperLimit', 100, label='')]), R('Model', 0)])],
    'SampleTargetBalance': [K('Channel', CHANNELS, label=''), CK('AdjustAll', True), HI(),
                            CK('UseLow', False, 'Shadows'), ROW([L('LowSourceItem', BLACK, 'Sample'), L('LowTargetItem', BLACK, 'Target')]),
                            CK('UseMid', False, 'Midtones'), ROW([L('MidSourceItem', GRAY, 'Sample'), L('MidTargetItem', GRAY, 'Target')]),
                            CK('UseHigh', False, 'Highlights'), ROW([L('HighSourceItem', WHITE, 'Sample'), L('HighTargetItem', WHITE, 'Target')])],
    'ToneCurve': [K('ActiveChannel', CHANNELS, label=''), CU('Curve{ActiveChannel}', True)],
    'SMH': [S(k, 0, -100, 100) for k in ('Brightness', 'Contrast', 'Intensity', 'Highlights', 'Shadows', 'Midtones')],
    'ColorBalance': [K('Mode', ['Master', '3-Way Color'], 1, label=''), CK('Luminance', True),
                     WHEN('Mode', 0, [S('CR_Master', 0, -100, 100, g=('#00ffff', WHITE, '#ff0000')), S('MG_Master', 0, -100, 100, g=('#ff00ff', WHITE, '#00ff00')), S('YB_Master', 0, -100, 100, g=('#ffff00', WHITE, '#0000ff'))]),
                     WHEN('Mode', 1, sum([[H(r), S('CR_' + r, 0, -100, 100, g=('#00ffff', WHITE, '#ff0000')), S('MG_' + r, 0, -100, 100, g=('#ff00ff', WHITE, '#00ff00')), S('YB_' + r, 0, -100, 100, g=('#ffff00', WHITE, '#0000ff'))] for r in ('Shadows', 'Midtones', 'Highlights')], []))],
    'Gamma': [S('Level', 1.0, 0.1, 10, label='Gamma', step=0.1, dec=1, log=True)],
    'TintTemp': [S('Temperature', 5000, 2000, 8000, step=100, g=('#f7941d', '#a6c8f5')), S('Tint', 0, -100, 100, g=('#e6a3f5', '#a3f07a'))],
    'HueSaturationLightness': [SW('Channel', [('Master', 'rainbow'), ('Red', '#ff0000'), ('Yellow', '#ffff00'), ('Green', '#00ff00'), ('Cyan', '#00ffff'), ('Blue', '#0000ff'), ('Magenta', '#ff00ff'), ('Grayscale', '#808080')], 0, 'Channel'),
                               S('Hue{Channel}', 0, -180, 180, label='Hue', g=RAINBOW), S('Saturation{Channel}', 0, -100, 100, label='Saturation', g=(GRAY, '#00ffff')), S('Lightness{Channel}', 0, -100, 100, label='Lightness', g=(BLACK, '#00ffff', WHITE))],
    'Grayscale': [S('Reds', 40, -200, 300, label='Red', g=tri('#ff0000')), S('Yellows', 60, -200, 300, label='Yellow', g=tri('#ffff00')), S('Greens', 40, -200, 300, label='Green', g=tri('#00ff00')),
                  S('Cyans', 60, -200, 300, label='Cyan', g=tri('#00ffff')), S('Blues', 20, -200, 300, label='Blue', g=tri('#0000ff')), S('Magentas', 80, -200, 300, label='Magenta', g=tri('#ff00ff')),
                  CK('SplitToneTintEnabled', False), EN('SplitToneTintEnabled', [H('Shadows'), S('ShadowsHue', 42, 0, 360, label='Hue'), S('ShadowsSaturation', 0, 0, 100, label='Saturation'),
                                                                               H('Highlights'), S('HighlightsHue', 42, 0, 360, label='Hue'), S('HighlightsSaturation', 0, 0, 100, label='Saturation')])],
    'Vibrance': [S('Vibrance', 0, -100, 100), S('Saturation', 0, -100, 100)],
    'SelectiveClr': [SW('Spectrum', [('Reds', '#ff0000'), ('Yellows', '#ffff00'), ('Greens', '#00ff00'), ('Cyans', '#00ffff'), ('Blues', '#0000ff'), ('Magentas', '#ff00ff'), ('Shadows', '#3a3a3a'), ('Midtones', '#808080'), ('Highlights', '#f4f4f4')], 0, 'Color', split=6),
                     R('Mode', 0), S('Cyan{Spectrum}', 0, -100, 100, label='Cyan'), S('Magenta{Spectrum}', 0, -100, 100, label='Magenta'), S('Yellow{Spectrum}', 0, -100, 100, label='Yellow'), S('Black{Spectrum}', 0, -100, 100, label='Black')],
    'ReplaceColorsV2': [U('Wheel'), ROW([L('ColorItem', None, 'Original'), L('ReplaceColorItem', None, 'New')]), EN('ColorItem', [CK('SingleColor', False), S('Smoothness', 50, 0, 100, label='Smooth')]),
                        S('Hue', 0, -180, 180, g=RAINBOW), S('Saturation', 0, -100, 100, g=(GRAY, '#00ffff')), S('Lightness', 0, -100, 100, g=(BLACK, '#00ffff', WHITE))],
    'Desaturate': [],
    'ChannelMixer': [K('InputColourModel', ['RGB'], label=''), K('OutputChannelRGB', ['Red', 'Green', 'Blue'], label=''), CK('PreviewChannel', False),
                     S('RedChannel{OutputChannelRGB}', 0, -200, 200, label='Red', g=tri('#ff0000')), S('GreenChannel{OutputChannelRGB}', 0, -200, 200, label='Green', g=tri('#00ff00')), S('BlueChannel{OutputChannelRGB}', 0, -200, 200, label='Blue', g=tri('#0000ff'))],
    # Art Strokes
    'Charcoal': [S('Size', 5, 1, 10), S('Edge', 0, 0, 10)],
    'ConteCrayon': [H('Conté Colors'), ROW([T('Black', BLACK, True), T('White', WHITE), T('Sanguine', '#c0704f'), T('Bistre', '#7a6450'), T('Sepia', '#a89c7c')]), L('PaperColorItem', '#ece8d2'), S('Pressure', 50, 0, 100), S('Texture', 0, 0, 100)],
    'Crayon': [S('Size', 12, 1, 20), S('Outline', 25, 0, 50)],
    'Cubist': [S('Size', 10, 1, 20), S('Brightness', 50, 1, 100), L('PaperColorItem', BLACK)],
    'Dabble': [K('Style', ['Default'], label='Style'), S('Size', 1, 1, 10), SD()],
    'Impressionist': [R('Style', 0), S('Detail', 33, 0, 100, label='Stroke'), S('Coloration', 5, 0, 100), S('Brightness', 50, 0, 100)],
    'PaletteKnife': [S('Size', 30, 1, 50), S('Softness', 0, 0, 10), A('Angle', 180)],
    'Pastels': [R('PastelType', 0), S('Size', 5, 1, 20), S('HueVariation', 30, 0, 100)],
    'PenInk': [R('Style', 0), S('Density', 75, 1, 100), S('InkPools', 75, 1, 100)],
    'Pointillist': [S('Size', 5, 1, 10), S('Brightness', 50, 1, 100)],
    'ScraperBoard': [R('ScraperMode', 0), S('Density', 25, 1, 40), S('Size', 5, 1, 40)],
    'SketchPad': [R('PencilType', 0), S('Style', 25, 0, 50), S('Lead', 75, 1, 100, label='Lead'), S('Outline', 25, 0, 100)],
    'Watercolor': [S('BrushSize', 1, 1, 10), S('Granulation', 50, 1, 100), S('WaterAmount', 50, 1, 100), S('Bleed', 35, 1, 100), S('Brightness', 25, 1, 100)],
    'WaterMarker': [R('VariationMode', 0), S('Size', 1, 1, 10), S('ColorVariation', 25, 0, 50)],
    'WavePaper': [R('ColorMode', 0), S('Pressure', 10, 1, 20)],
    # Blur
    'TuneBlur': [TH(['Gaussian', 'Smooth', 'Directional Smooth', 'Soften']), S('Step', 10, 1, 100)],
    'DirectionalSmooth': [S('Percentage', 50, 0, 100)],
    'Feather': [S('Width', 20, 1, 200, log=True), R('Mode', 0)],
    'GaussianBlur': [S('Radius', 5.0, 0.1, 250, step=0.1, dec=1, log=True)],
    'JaggyDespeckle': [S('Width', 3, 1, 5), LK('Symmetric', True), S('Height', 3, 1, 5)],
    'LowPass': [S('Percentage', 100, 0, 100), S('Radius', 1, 1, 20)],
    'MotionBlur': [S('Distance', 5, 1, 1000, log=True), A('Direction', 0), R('Sampling', 0)],
    'RadialBlur': [X(), S('Amount', 5, 1, 100)],
    'SmartBlur': [S('Detail', 50, 0, 100, label='Amount', step=5)],
    'Smooth': [S('Percentage', 50, 0, 100)],
    'Soften': [S('Percentage', 50, 1, 100)],
    'Zoom': [X(), S('Amount', 5, 1, 100)],
    # Camera
    'Colorize': [S('Hue', 0, 0, 360, g=RAINBOW), S('Saturation', 0, 0, 100, g=(WHITE, '#ff0000'))],
    'Diffuse': [S('Level', 50, 1, 100)],
    'PhotoFilter': [L('ColorItem', '#ff0000', dropper=False), S('Density', 100, 0, 100, g=(WHITE, BLACK)), CK('PreserveLuminosity', False)],
    'LensFlare': [X(0.3, 0.3), N('AspectRatio', 100, 0, 200),
                  TABS('LensFlarePageSelected', [('flare_flares', 'Flares', [K('FlaresProperties', ['General', 'Halo', 'Reflections'], label=''),
                                                                            WHEN('FlaresProperties', 0, [S('FlareSize', 10), S('FlareBrightness', 23), S('FlareFallof', 18), L('FlareColorItem', '#ff0000')]),
                                                                            WHEN('FlaresProperties', 1, [S('HaloSize', 30), S('HaloBrightness', 20), L('HaloColorItem', '#ffc878')]),
                                                                            WHEN('FlaresProperties', 2, [S('ReflectionSize', 20), S('ReflectionBright', 30)])]),
                                                 ('flare_rays', 'Rays', [K('RaysProperties', ['Rays', 'Streaks', 'Anamorphic'], label=''),
                                                                        WHEN('RaysProperties', 0, [L('RayColorItem', '#ffe6c8'), ROW([N('RayRNoise', 0), N('RaySNoise', 0)]), S('RayCount', 12), S('RayBrightness', 30), S('RayRotation', 0, 0, 360)]),
                                                                        WHEN('RaysProperties', 1, [S('StreakBrightness', 0), S('RaySharpness', 50)]),
                                                                        WHEN('RaysProperties', 2, [S('AnamorphicBrightness', 0), S('AnamorphicRotation', 0, 0, 360), L('AnamorphicColorItem', '#78b4ff')])])]),
                  PR()],
    'Lighting': [ROW([N('SourceX', 0.25, 0, 1, label='X:', step=0.01, dec=2), N('SourceY', 0.25, 0, 1, label='Y:', step=0.01, dec=2)]), P,
                 TABS('LightingPageSelected', [('light_source', 'Light Source', [CK('EnableLightSource', True), R('LightType', 0), S('Brightness', 100, 0, 200), S('ConeSize', 90, 1, 180, unit='°'), S('Opacity', 100), S('Edge', 50), A('Angle', 45, quarter=True), L('LightColorItem', WHITE)]),
                                               ('light_atmosphere', 'Atmosphere', [CK('EnableAtmosphere', True), S('AtmosphereBrightness', 35), L('AtmosphereColorItem', WHITE), S('Exposure', 0, -100, 100)]),
                                               ('light_texture', 'Image Texture', [K('Channel', ['None', 'Red', 'Green', 'Blue'], label='Channel'), S('Contrast', 0), S('Relief', 0)])]),
                 PR()],
    'Sepia': [S('Amount', 50, 1, 100)],
    'SpotFilter': [X(), S('Sharpenraduis', 40), S('Feathering', 50), S('Lightfalloff', 0), PR(['Last Used'])],
    'TimeMachine': [ST('Style', ['Daguerreotype', 'Albumen', 'Cyanotype', 'Platinum', 'Early Color', 'Box Camera', 'Cross Process'], 0), S('Intensity', 80, 0, 100), CK('PhotoEdges', True)],
    # Color Transform
    'BitPlanes': [S('Red', 0, 0, 7), S('Green', 0, 0, 7), S('Blue', 0, 0, 7), CK('AllPlanes', True)],
    'Halftone': [S('CAngle', 15, 0, 360, unit='°'), S('MAngle', 75, 0, 360, unit='°'), S('YAngle', 0, 0, 360, unit='°'), S('KAngle', 45, 0, 360, unit='°'), S('MaxRadius', 5, 2, 10)],
    'Psychedelic': [S('Level', 127, 1, 255)],
    'Solarize': [S('Level', 127, 0, 255)],
    # Contour
    'EdgeDetect': [R('ColorMode', 0), L('BackgroundColorItem', BLACK, label=''), S('Sensitivity', 1, 1, 10)],
    'FindEdges': [R('EdgeType', 0), S('Level', 50, 0, 100)],
    'TraceContour': [S('Level', 127, 1, 255), R('EdgeType', 0)],
    'LocalEqualize': [S('Width', 50, 3, 255), LK('Square', True), S('Height', 50, 3, 255)],
    # Correction
    'DustScratch': [S('Radius', 2, 1, 20), S('Threshold', 64, 0, 255)],
    'TuneSharpness': [TH(['Unsharp Mask', 'Adaptive Unsharp', 'Sharpen', 'Directional Sharpen']), S('Percentage', 10, 1, 100), S('Background', 50, 0, 100)],
    # Creative
    'NNArtStyle': [K('CurrentStylePreset', ['Smooth Acrylic', 'Graphite', 'Pastel Mosaic', 'Neon', 'Woodcut', 'Lava Lamp'], label='Style'), S('Intensity', 100, 0, 100), R('Detail', 2)],
    'Crystallize': [S('Size', 10, 3, 100)],
    'Fabric': [K('Style', ['Needlepoint', 'Rug Hooking', 'Quilt', 'Strings', 'Ribbons', 'Tissue Collage'], 2, label='Style'), S('Size', 10, 1, 100), S('Complete', 100, 1, 100), S('Brightness', 50, 0, 100), A('Rotation', 0)],
    'Frame': [U('Frames'), S('Horizontal', 100, 1, 140), LK('Square', True), S('Vertical', 100, 1, 140), A('Rotate', 0), L('ColorItem', BLACK), S('Opacity', 100), S('BlurFeather', 0, label='Blur/Feather'),
              ROW([T('HFlip', 'flip_h'), T('VFlip', 'flip_v')]), PR()],
    'GlassBlock': [S('BlockWidth', 20, 1, 100), LK('Square', False), S('BlockHeight', 20, 1, 100)],
    'Mosaic': [S('Size', 10, 2, 20), L('BackgroundColorItem', '#bdb76b'), CK('Vignette', False)],
    'Scatter': [S('Horizontal', 10, 0, 100), LK('Square', True), S('Vertical', 10, 0, 100)],
    'SmokedGlass': [S('Tint', 40), S('Level', 90), L('ColorItem', BLACK)],
    'StainedGlass': [S('Size', 20, 3, 100), S('LightIntensity', 2, 1, 10), N('Thickness', 10, 1, 50), L('BorderColorItem', BLACK), CK('3DLighting', False)],
    'Vignette': [R('ColorMode', 0), L('ColorItem', WHITE, label=''), R('Shape', 0), S('Offset', 100, 0, 150), S('Fade', 75, 0, 100)],
    'Vortex': [X(), K('Style', ['Brushed', 'Layered', 'Thick', 'Thin'], label='Style', inline=True), S('Size', 1, 1, 100), A('InnerDirection', 180), A('OuterDirection', 180)],
    # Custom
    'BandPass': [S('InWeight', 100), S('MidWeight', 100)],
    'BumpMap': [TABS('BumpMapPageSelected', [('bump_map', 'Bump Map', [R('ScalingMode', 0), EN('ScalingMode', [N('TileWidth', 2, 1, 50), LK('SquareTile', True), N('TileHeight', 2, 1, 50)]), TX('CurrentBumpMapImage'), CK('Invert', False), CK('Smooth', False)]),
                                             ('bump_surface', 'Surface', [S('ScaleFactor', 100, 1, 200), S('Floor', 0, 0, 255), S('Ceiling', 255, 0, 255), S('Highlight', 50)]),
                                             ('bump_light', 'Lighting', [L('DirectionalLightColorItem', WHITE), A('Direction', 135), A('Declination', 45, quarter=True), S('DirectionalLightBrightness', 100), CK('AutoComputeBrightness', False),
                                                                         L('AmbientLightColorItem', WHITE), S('AmbientLightBrightness', 50)])]),
                PR(['Last Used'])],
    'UserDefined': [U('Kernel'), N('Divisor', 1, 1, 999), N('Offset', 0, -255, 255), CK('AutoDivisor', True), CK('PreserveColors', False), CK('SymmetricKernel', True), PR(['Last Used'])],
    # Distort
    'Blocks': [S('Width', 10, 1, 100), LK('Square', True), S('Height', 10, 1, 100), S('Offset', 50, 0, 100), ROW([K('UndefinedArea', ['Original Image', 'Inverse Image', 'Black', 'White', 'Other'], 4, label='Undefined Areas'), L('ColorItem', WHITE, label='')])],
    'Displace': [R('ScaleMode', 0), H('Scale'), S('HorizontalScale', 10), S('VerticalScale', 10), K('UndefinedArea', ['Repeat Edges', 'Wrap Around'], label='Undefined Areas'), TX('CurrentDisplaceImage', label='')],
    'MeshWarp': [U('Mesh'), N('Gridlines', 4, 2, 10), PR()],
    'Offset': [H('Shift'), S('Horizontal', 50, -100, 100), S('Vertical', 50, -100, 100), CK('AsPercent', True), ROW([K('UndefinedArea', ['Wrap Around', 'Repeat Edges', 'Other Color'], label='Undefined Areas'), L('ColorItem', WHITE, label='')])],
    'Pixelate': [R('Mode', 0), X(), H('Adjust'), S('Width', 10, 1, 100), S('Height', 10, 1, 100), S('Opacity', 100)],
    'Ripple': [P, H('Primary Wave'), S('Period', 20, 1, 100), S('Amplitude', 20, 1, 100), R('Optimize', 0), EN('PerpendicularWave', [S('PerpAmplitude', 20, 1, 100, label='Amplitude')]), CK('PerpendicularWave', False), A('Angle', 90), CK('DistortRipple', False)],
    'Shear': [CU('Curve'), K('EditStyle', ['Curve', 'Linear', 'Freehand'], label='Edit Style'), R('Orientation', 1), S('Scale', 50), H('Undefined Areas'), ROW([K('EdgeStyle', ['Wrap Around', 'Repeat Edges', 'Other Color'], 2, label='Edges style'), L('ColorItem', WHITE, label='', dropper=False)]), PR()],
    'Swirl': [P, X(), R('Direction', 0), R('Optimize', 0), H('Angle'), S('Rotation', 0, 0, 10), S('Degree', 90, 0, 359, unit='°')],
    'Tile': [S('TilesPerRow', 10, 1, 100), LK('Square', True), S('TilesPerCol', 10, 1, 100), S('PercentOverlap', 25, 0, 100)],
    'WetPaint': [S('Wetness', 45, -50, 50), S('Percent', 100, 0, 100)],
    'Whirlpool': [S('Spacing', 20, 5, 200), CK('Warp', True), S('SmearLength', 9, 3, 30), S('Twist', 70, 0, 90), S('StreakDetail', 60, 0, 100), PR()],
    'Wind': [S('Strength', 75), S('Opacity', 100), A('Angle', 0)],
    # Noise
    'TuneNoise': [TH(['More Spike', 'More Gaussian', 'More Uniform', 'Diffuse', 'Minimum', 'Median', 'Maximum', 'Jaggy Despeckle', 'Remove Noise']), S('Level', 50), S('Density', 50)],
    'AddNoise': [R('NoiseMode', 0), S('Level', 50), S('Density', 50), R('ColorMode', 0), L('ColorItem', BLACK, label='')],
    'ThreeDStereoNoise': [S('DepthValue', 3, 1, 10, label='Depth'), CK('ShowDots', True)],
    'Maximum': [S('Percentage', 100), S('Radius', 1, 1, 20)],
    'Median': [S('Radius', 1, 1, 20)],
    'Minimum': [S('Percentage', 100), S('Radius', 1, 1, 20)],
    'RemoveMoire': [S('Amount', 6, 0, 10), R('OptimizeMode', 0), N('OutputResolution', 150, 1, 1200, label='Output', unit='dpi')],
    'RemoveNoise': [EN('!Auto', [S('Threshold', 127, 0, 255)]), CK('Auto', True)],
    # Sharpen
    'AdaptiveUnsharp': [S('Level', 50, 1, 100)],
    'DirectionalSharpen': [S('Percentage', 50, 0, 100)],
    'HighPass': [S('Percentage', 100, 1, 100), S('Radius', 1, 1, 20)],
    'Sharpen': [S('EdgeLevel', 25, 0, 100), CK('PreserveColors', False), S('Threshold', 0, 0, 255)],
    'UnsharpMask': [S('Percentage', 100, 1, 500, log=True), S('Radius', 1, 1, 20), S('Threshold', 0, 0, 255)],
    # Texture
    'BrickWall': [S('Roughness', 25), N('BrickWidth', 60, 1, 500), N('BrickHeight', 20, 1, 500), CK('MaintainAspectRatio', False), N('GroutWidth', 5, 0, 100), A('LightDirection', 315)],
    'Bubbles': [S('Diameter', 50, 1, 500), S('Coverage', 50), A('LightDirection', 315), CK('Refraction', True)],
    'Canvas': [S('Transparency', 100), S('Emboss', 100, 0, 200), S('XOffset', 0, -100, 100), S('YOffset', 0, -100, 100), R('Mode', 0), S('TileOffset', 0), TX('CurrentCanvasImage')],
    'CobbleStone': [S('Roughness', 10), S('Size', 20, 1, 100), N('GroutWidth', 10, 0, 100), A('Direction', 315), CK('Warp', False)],
    'ElephantSkin': [S('Age', 20, 1, 100), SD('Variation'), L('ColorItem', BLACK)],
    'Etching': [S('Detail', 85), S('Depth', 50), A('LightDirection', 315), L('SurfaceColorItem', GRAY)],
    'Plastic': [S('Highlight', 90), S('Depth', 10), S('Smoothness', 30), A('LightDirection', 315), L('LightColorItem', WHITE)],
    'PlasterWall': [S('Detail', 50, 1, 100), S('Brightness', 50, 1, 100), SD()],
    'ReliefSculpture': [S('Detail', 85), S('Depth', 60), S('Smoothness', 15), A('LightDirection', 315), L('SurfaceColorItem', GRAY)],
    'ScreenDoor': [R('BackgroundColor', 0), S('Density', 40, 0, 60), S('Softness', 85), S('Brightness', 100)],
    'Stone': [S('Roughness', 100, 1, 100), S('Detail', 100, 1, 100), A('LightDirection', 315), CK('Invert', False), PR(['Last Used'])],
    'Underpainting': [S('Amount', 10, 1, 20), S('Brightness', 50)],
    # Transform
    'DeInterlace': [R('ScanLine', 0, label='Scan Lines'), R('Replacement', 1, label='Replacement Method')],
    'Invert': [],
    'Posterize': [S('Level', 4, 2, 32)],
    'Threshold': [HI(True), ROW([N('Low', 0, 0, 255, label='Low-level'), N('Mid', 127, 0, 255, label='Threshold'), EN('Level=2', [N('High', 255, 0, 255, label='High-level')])]),
                  K('Channel', ['RGB Channels', 'Red Channel', 'Green Channel', 'Blue Channel'], label='Channel'), R('Level', 0, label='Threshold'),
                  EN('!AutoClip', [N('ClipPercent', 5, 0, 50, label='Histogram Display Clipping', unit='%')]), CK('AutoClip', True, 'Auto')],
}

# the extra starting values of settings that depend on another (Channel Mixer: each output
# channel starts as itself)
INIT = {
    'ChannelMixer': [('RedChannel0', 100), ('GreenChannel1', 100), ('BlueChannel2', 100)],
}
# the effects without a dialog command (they apply at once)
DIRECT = {'Desaturate': ('25f5d0f5-ccce-40e0-993c-e07ce61d1c96', 'Desaturate'), 'Invert': ('072684d2-fbeb-440e-9747-875fa76a4eea', 'Invert Colors')}

# ------------------------------------------------------------------------------ Rust


def rs(s):
    return '"' + (s or '').replace('\\', '\\\\').replace('"', '\\"') + '"'


def num(v):
    v = float(v)
    return repr(v) if v != int(v) else '%d.0' % int(v)


def rgb(h):
    if h is None:
        return 'None'
    return 'Some([%d, %d, %d])' % tuple(C3(h))


def grad(g):
    if g is None:
        return 'Grad::None'
    if g == RAINBOW:
        return 'Grad::Hue'
    if len(g) == 2:
        return 'Grad::Two([%d, %d, %d], [%d, %d, %d])' % tuple(C3(g[0]) + C3(g[1]))
    return 'Grad::Three([%d, %d, %d], [%d, %d, %d], [%d, %d, %d])' % tuple(C3(g[0]) + C3(g[1]) + C3(g[2]))


missing = []


def label_of(sp, c):
    if c.get('label') is not None:
        return c['label']
    base = c['key'].split('{')[0]
    d = sp.get(base) or sp.get(c['key'])
    if d and d['label']:
        return d['label']
    missing.append(c['key'])
    return re.sub(r'(?<=[a-z])(?=[A-Z])', ' ', base)


def ctl(sp, c, ind):
    k = c['k']
    pad = ' ' * ind
    if k == 'Slider':
        return f"Ctl::Slider {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, min: {num(c['lo'])}, max: {num(c['hi'])}, step: {num(c['step'])}, dec: {c['dec']}, log: {str(c['log']).lower()}, def: {num(c['d'])}, unit: {rs(c['unit'])}, grad: {grad(c['g'])} }}"
    if k == 'Spin':
        return f"Ctl::Spin {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, min: {num(c['lo'])}, max: {num(c['hi'])}, step: {num(c['step'])}, dec: {c['dec']}, def: {num(c['d'])}, unit: {rs(c['unit'])} }}"
    if k == 'Angle':
        return f"Ctl::Angle {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, def: {num(c['d'])}, quarter: {str(c['quarter']).lower()} }}"
    if k == 'Radio':
        d = sp.get(c['key'])
        opts = c['opts'] or (d['opts'] if d else None)
        if not opts:
            missing.append(c['key'] + ' (options)')
            opts = ['?']
        return f"Ctl::Radio {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, opts: &[{', '.join(rs(o) for o in opts)}], def: {c['d']} }}"
    if k == 'Icons':
        return f"Ctl::Icons {{ key: {rs(c['key'])}, label: {rs(c['label'])}, icons: &[{', '.join('(%s, %s)' % (rs(a), rs(b)) for a, b in c['icons'])}], def: {c['d']} }}"
    if k == 'Check':
        return f"Ctl::Check {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, def: {str(c['d']).lower()} }}"
    if k == 'Combo':
        return f"Ctl::Combo {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, opts: &[{', '.join(rs(o) for o in c['opts'])}], def: {c['d']}, inline: {str(c['inline']).lower()} }}"
    if k == 'Color':
        return f"Ctl::Color {{ key: {rs(c['key'])}, label: {rs(label_of(sp, c))}, def: {rgb(c['d'])}, dropper: {str(c['dropper']).lower()} }}"
    if k == 'Toggle':
        sw = c['swatch']
        if sw.startswith('#'):
            return f"Ctl::Toggle {{ key: {rs(c['key'])}, def: {str(c['d']).lower()}, swatch: [{', '.join(str(x) for x in C3(sw))}], icon: \"\" }}"
        return f"Ctl::Toggle {{ key: {rs(c['key'])}, def: {str(c['d']).lower()}, swatch: [0, 0, 0], icon: {rs(sw)} }}"
    if k == 'Center':
        return f"Ctl::Center {{ x: {num(c['x'])}, y: {num(c['y'])} }}"
    if k == 'Lock':
        return f"Ctl::Lock {{ key: {rs(c['key'])}, def: {str(c['d']).lower()} }}"
    if k == 'Seed':
        return f"Ctl::Seed {{ key: {rs(c['key'])}, label: {rs(c['label'])} }}"
    if k == 'Heading':
        return f"Ctl::Heading({rs(c['text'])})"
    if k == 'Thumbs':
        return f"Ctl::Thumbs {{ ops: &[{', '.join(rs(o) for o in c['ops'])}] }}"
    if k == 'Styles':
        return f"Ctl::Styles {{ key: {rs(c['key'])}, opts: &[{', '.join(rs(o) for o in c['opts'])}], def: {c['d']} }}"
    if k == 'Swatches':
        sw = ', '.join('(%s, %s)' % (rs(n), 'None' if h == 'rainbow' else rgb(h)) for n, h in c['opts'])
        return f"Ctl::Swatches {{ key: {rs(c['key'])}, label: {rs(c['label'])}, opts: &[{sw}], def: {c['d']}, split: {c['split']} }}"
    if k == 'Presets':
        return f"Ctl::Presets {{ opts: &[{', '.join(rs(o) for o in c['opts'])}] }}"
    if k == 'Texture':
        return f"Ctl::Texture {{ key: {rs(c['key'])}, label: {rs(c['label'])} }}"
    if k == 'Curve':
        return f"Ctl::Curve {{ key: {rs(c['key'])}, hist: {str(c['hist']).lower()} }}"
    if k == 'Histogram':
        return f"Ctl::Histogram {{ markers: {str(c['markers']).lower()} }}"
    if k in ('When', 'Enable'):
        inner = ',\n'.join(pad + '    ' + ctl(sp, x, ind + 4) for x in c['ctls'])
        if k == 'When':
            return f"Ctl::When {{ key: {rs(c['key'])}, val: {num(c['val'])}, ctls: &[\n{inner},\n{pad}] }}"
        return f"Ctl::Enable {{ key: {rs(c['key'])}, ctls: &[\n{inner},\n{pad}] }}"
    if k == 'Row':
        inner = ',\n'.join(pad + '    ' + ctl(sp, x, ind + 4) for x in c['ctls'])
        return f"Ctl::Row(&[\n{inner},\n{pad}])"
    if k == 'Tabs':
        tabs = []
        for icon, tip, cs in c['tabs']:
            inner = ',\n'.join(pad + '        ' + ctl(sp, x, ind + 8) for x in cs)
            tabs.append(f"{pad}    ({rs(icon)}, {rs(tip)}, &[\n{inner},\n{pad}    ])")
        return f"Ctl::Tabs {{ key: {rs(c['key'])}, tabs: &[\n" + ',\n'.join(tabs) + f",\n{pad}] }}"
    return f"Ctl::{k}"


def main():
    guids = effect_guids()
    guids.update({k: v for k, v in DIRECT.items()})
    out = ['//! The page of every bitmap effect of the Effects menu in the Properties docker (Bitmap',
           '//! Effects): its settings in the reference order, with their labels, ranges and defaults.',
           '//!',
           '//! Generated by `python xtask/fxgen.py` from the reference definition; do not edit by hand.',
           '',
           'use crate::fx::{Ctl, FxDef, Grad};',
           '',
           'pub static FX: &[FxDef] = &[']
    for eid, page in PAGES.items():
        if eid not in guids:
            raise SystemExit('no menu command for ' + eid)
        g, name = guids[eid]
        sp = spec(eid + 'EffectDS')
        body = ',\n'.join('            ' + ctl(sp, c, 12) for c in page)
        init = ', '.join('(%s, %s)' % (rs(a), num(b)) for a, b in INIT.get(eid, []))
        out.append('    FxDef {')
        out.append(f'        id: {rs(eid)},')
        out.append(f'        guid: {rs(g)},')
        out.append(f'        name: {rs(name)},')
        out.append(f'        init: &[{init}],')
        out.append('        ctls: &[' + ('\n' + body + ',\n        ' if page else '') + '],')
        out.append('    },')
    out.append('];')
    missing_ids = sorted(set(guids) - set(PAGES))
    path = os.path.join(ROOT, 'crates', 'anyvector', 'src', 'fxdata.rs')
    open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')
    print(f'{len(PAGES)} effects -> {path}')
    if missing:
        print('labels not in the definition (named from the setting):', ', '.join(sorted(set(missing))))
    if missing_ids:
        print('menu effects without a page:', ', '.join(missing_ids))


if __name__ == '__main__':
    main()
