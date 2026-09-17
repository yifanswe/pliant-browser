"""Render concept illustrations. Requires Pillow; no network or private data."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets'
OUT.mkdir(exist_ok=True)
FONT_DIRS = [Path('/System/Library/Fonts/Supplemental'), Path('/usr/share/fonts/truetype/dejavu')]

def font(size, bold=False):
    names = ['Arial Bold.ttf', 'DejaVuSans-Bold.ttf'] if bold else ['Arial.ttf', 'DejaVuSans.ttf']
    for folder in FONT_DIRS:
        for name in names:
            if (folder / name).exists():
                return ImageFont.truetype(str(folder / name), size)
    raise RuntimeError('Install no fonts automatically; supply Arial or DejaVu Sans.')

INK = '#1C302C'
MUTED = '#647570'
PAPER = '#F6F4EC'
GREEN = '#BCE9B1'
BLUE = '#C6D9F4'
ORANGE = '#F3D2AE'
WHITE = '#FFFEF9'
LINE = '#D5DCD1'

def canvas(h):
    im = Image.new('RGB', (1600, h), PAPER)
    return im, ImageDraw.Draw(im)

def text(d, pos, value, size=24, fill=INK, bold=False):
    d.text(pos, value, font=font(size, bold), fill=fill)

def box(d, xy, fill=WHITE, radius=22, outline=None, width=2):
    d.rounded_rectangle(xy, radius, fill=fill, outline=outline, width=width)

def pill(d, x, y, label, fill=GREEN, size=18):
    f = font(size, True)
    w = d.textbbox((0, 0), label, font=f)[2] + 28
    box(d, (x, y, x+w, y+34), fill, 17)
    text(d, (x+14, y+6), label, size, bold=True)

def line(d, xy, fill=LINE, width=2):
    d.line(xy, fill=fill, width=width)

def browser(d, x, y, w, h, kind):
    box(d, (x+6,y+10,x+w+6,y+h+10), '#E3E5DB', 20)
    box(d, (x,y,x+w,y+h), WHITE, 20, LINE)
    for i,c in enumerate(['#DB9B8A','#E4C982','#98BE97']):
        d.ellipse((x+18+i*19,y+17,x+28+i*19,y+27), fill=c)
    text(d,(x+92,y+14),'pliant / '+kind,17, MUTED)
    line(d,(x,y+45,x+w,y+45))
    if kind == 'workspace':
        box(d,(x+10,y+54,x+145,y+h-12),'#EDF1E7',12)
        text(d,(x+24,y+69),'Workspaces',16,bold=True)
        for j,label in enumerate(['Research','  Reading','  Sketches','Personal']):
            yy=y+110+j*39
            if j==0: box(d,(x+18,yy-5,x+137,yy+27),GREEN,7)
            text(d,(x+27,yy),label,15)
        xx=x+165
        text(d,(xx,y+77),'A place to think.',23,bold=True)
        for j,(label,col) in enumerate([('Notes',BLUE),('Sources',ORANGE)]):
            box(d,(xx,y+128+j*96,x+w-18,y+208+j*96),col,12)
            text(d,(xx+16,y+144+j*96),label,18,bold=True)
            line(d,(xx+16,y+181+j*96,x+w-43,y+181+j*96),'#8FABA0')
    else:
        for j,label in enumerate(['Reading','Project','Notes']):
            box(d,(x+12+j*127,y+55,x+131+j*127,y+88),BLUE if j==0 else '#EEF0E9',7)
            text(d,(x+25+j*127,y+63),label,15)
        box(d,(x+15,y+100,x+w-15,y+132),'#F0F1EB',8)
        text(d,(x+28,y+108),'Search or enter address',14,MUTED)
        text(d,(x+29,y+156),'Keep it familiar.',26,bold=True)
        for j in range(4):
            line(d,(x+29,y+213+j*27,x+w-35-(j%2)*45,y+213+j*27),'#D7DED4',5)
        pill(d,x+29,y+h-57,'YOUR ROUTING PLUGIN',ORANGE,13)

def hero():
    im,d=canvas(980)
    pill(d,64,42,'PLIANT / DESIGN PROPOSAL')
    text(d,(64,104),'Your AI. Your browser.',76,bold=True)
    text(d,(68,205),'One foundation. Entirely different ways to browse.',30,MUTED)
    browser(d,64,306,475,356,'workspace')
    browser(d,582,306,439,356,'classic')
    box(d,(1114,286,1396,698),INK,36)
    box(d,(1123,296,1387,688),WHITE,29)
    box(d,(1193,305,1317,325),INK,10)
    text(d,(1144,351),'My spaces',26,bold=True)
    for j,(label,col) in enumerate([('Research',GREEN),('Reading',BLUE),('Personal',ORANGE)]):
        box(d,(1142,402+j*66,1368,456+j*66),col,12)
        text(d,(1157,418+j*66),label,20,bold=True)
    box(d,(1142,618,1368,667),'#EDF1E7',12)
    text(d,(1158,635),'Spaces     +     Search',16)
    text(d,(139,696),'A workspace-first desktop',20,MUTED)
    text(d,(635,696),'A familiar desktop',20,MUTED)
    text(d,(1150,716),'A touch-first mobile',20,MUTED)
    for x,y in [(302,735),(801,735),(1255,754)]:
        line(d,(x,y,x,784), '#9BAE9A',3)
    line(d,(302,784,1255,784),'#9BAE9A',3)
    line(d,(800,784,800,813),'#9BAE9A',3)
    box(d,(64,813,1536,916),INK,22)
    text(d,(94,834),'SHARED FOUNDATION',19,GREEN,True)
    text(d,(94,867),'Engine   /   Profiles & data   /   Plugin contracts   /   Safety   /   Upgrades',26,WHITE)
    text(d,(66,943),'CONCEPT ILLUSTRATION — NOT A PRODUCT SCREENSHOT',15,MUTED)
    im.save(OUT/'hero.png', optimize=True)

def architecture():
    im,d=canvas(1080)
    pill(d,64,38,'PLIANT / ARCHITECTURE')
    text(d,(64,96),'Customize the experience.',57,bold=True)
    text(d,(64,164),'Keep the foundation dependable.',45,MUTED)
    # User-authored layer
    text(d,(68,258),'USER + THEIR CHOSEN AI',19,MUTED,True)
    for x,w,title,sub,col in [(64,460,'Complete UI','Desktop and mobile layouts',GREEN),(554,460,'Behavior plugins','Routing, providers, session policies',BLUE),(1044,492,'Personal state','Organization and preferences',ORANGE)]:
        box(d,(x,301,x+w,418),col,18)
        text(d,(x+24,323),title,29,bold=True)
        text(d,(x+24,371),sub,20)
    for x in [294,784,1290]: line(d,(x,419,x,462),'#9BAE9A',3)
    box(d,(64,462,1536,551),WHITE,18,LINE)
    text(d,(91,480),'STABLE, VERSIONED CONTRACTS',22,bold=True)
    text(d,(91,517),'State  /  Commands  /  Events  /  Replaceable service interfaces',21,MUTED)
    line(d,(800,552,800,592),'#9BAE9A',3)
    box(d,(64,592,1536,765),INK,22)
    text(d,(94,617),'TRUSTED CORE',21,GREEN,True)
    text(d,(94,659),'Engine abstraction + profile and data management',31,WHITE,True)
    text(d,(94,713),'Enforces permissions, isolation, lifecycle and resource limits',24,'#D4DFD7')
    box(d,(64,795,777,963),WHITE,20,LINE)
    text(d,(90,818),'Tests + developer tools',29,bold=True)
    text(d,(90,865),'Inspect, validate, preview, diagnose.',23,MUTED)
    text(d,(90,909),'Tests find failures. Runtime guards enforce limits.',20,MUTED)
    box(d,(807,795,1536,963),WHITE,20,LINE)
    text(d,(833,818),'Upgrades + recovery',29,bold=True)
    text(d,(833,865),'Migrate contracts. Preserve customizations.',23,MUTED)
    text(d,(833,909),'No silent replacement of privileged policies.',20,MUTED)
    text(d,(66,1007),'PROPOSED DESIGN • EXISTING WEB ENGINE • NATIVE PLATFORM ADAPTERS',17,MUTED)
    im.save(OUT/'architecture.png', optimize=True)

if __name__ == '__main__':
    hero()
    architecture()
    print('Rendered assets/hero.png and assets/architecture.png')
