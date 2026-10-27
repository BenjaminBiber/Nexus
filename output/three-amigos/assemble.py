from pathlib import Path
from PIL import Image
import subprocess

ROOT=Path(__file__).resolve().parent
MASKS=ROOT/'masks-birefnet'
MASKS.mkdir(exist_ok=True)
source=Image.open(r'C:\Users\benjamin.biber\Downloads\three-amigos-3amigos.gif')
frames=[]
durations=[]
paths=sorted(MASKS.glob('frame-*.png'))
assert len(paths)==source.n_frames
for i,p in enumerate(paths):
    source.seek(i)
    frame=source.convert('RGBA')
    alpha=Image.open(p).convert('RGBA').getchannel('A')
    assert alpha.getextrema()[0]==0
    frame.putalpha(alpha)
    frames.append(frame)
    durations.append(source.info.get('duration',100))
frames[0].save(ROOT/'three-amigos-transparent.webp',save_all=True,append_images=frames[1:],duration=durations,loop=0,lossless=True)
atlas=Image.new('RGB',(220*10,124*7),'white')
for i,f in enumerate(frames):
    atlas.paste(f,((i%10)*220,(i//10)*124),f.getchannel('A'))
palette=atlas.quantize(colors=255)
gif=[]
for f in frames:
    rgb=Image.new('RGB',f.size,'white')
    rgb.paste(f,mask=f.getchannel('A'))
    indexed=rgb.quantize(palette=palette,dither=Image.Dither.NONE)
    indexed.paste(255,mask=f.getchannel('A').point(lambda a:255 if a<128 else 0))
    indexed.info['transparency']=255
    gif.append(indexed)
gif[0].save(ROOT/'three-amigos-transparent.gif',save_all=True,append_images=gif[1:],duration=durations,loop=0,transparency=255,disposal=2,optimize=False)
contact=Image.new('RGB',(220*3,124*2),'#e5e8ee')
for n,i in enumerate([0,14,28,42,56,69]):
    contact.paste(frames[i],((n%3)*220,(n//3)*124),frames[i].getchannel('A'))
contact.resize((1320,496),Image.Resampling.NEAREST).save(ROOT/'preview.jpg')
result=Image.open(ROOT/'three-amigos-transparent.gif')
assert result.n_frames==70
assert result.convert('RGBA').getpixel((0,0))[3]==0
print('Verified:',result.n_frames,'frames',result.size,'duration',sum(durations),'ms')
