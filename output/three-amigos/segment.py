import os
import sys
os.environ['OMP_NUM_THREADS']='4'
sys.path.insert(0,r'C:\Users\benjamin.biber\.cache\mariachi-bgremove-python')
from pathlib import Path
from PIL import Image
import numpy as np
import onnxruntime as ort

root=Path(__file__).resolve().parent
source=Image.open(r'C:\Users\benjamin.biber\Downloads\three-amigos-3amigos.gif')
out=root/'masks-birefnet'
out.mkdir(exist_ok=True)
print('Loading segmentation model',flush=True)
options=ort.SessionOptions()
options.intra_op_num_threads=4
options.inter_op_num_threads=1
session=ort.InferenceSession(r'C:\Users\benjamin.biber\.u2net\birefnet-general.onnx',sess_options=options,providers=['CPUExecutionProvider'])
def remove(image):
    pixels=np.asarray(image.resize((1024,1024),Image.Resampling.LANCZOS)).astype(np.float32)/255
    pixels=(pixels-np.array([.485,.456,.406],dtype=np.float32))/np.array([.229,.224,.225],dtype=np.float32)
    prediction=session.run(None,{session.get_inputs()[0].name:pixels.transpose(2,0,1)[None]})[0][0,0]
    prediction=1/(1+np.exp(-np.clip(prediction,-80,80)))
    prediction=(prediction-prediction.min())/(prediction.max()-prediction.min())
    alpha=Image.fromarray((prediction*255).astype('uint8')).resize(image.size,Image.Resampling.LANCZOS)
    result=image.convert('RGBA')
    result.putalpha(alpha)
    return result
indices=[0,14,28,42,56,69] if '--test' in sys.argv else range(source.n_frames)
if '--batch' in sys.argv:
    for start in range(0,source.n_frames,9):
        mosaic=Image.new('RGB',(660,372),'#808080')
        count=min(9,source.n_frames-start)
        for n in range(count):
            source.seek(start+n)
            mosaic.paste(source.convert('RGB'),((n%3)*220,(n//3)*124))
        cutout=remove(mosaic)
        for n in range(count):
            x,y=(n%3)*220,(n//3)*124
            cutout.crop((x,y,x+220,y+124)).save(out/f'frame-{start+n+1:03d}.png')
        print(f'Processed {start+count}/70',flush=True)
    indices=[]
for i in indices:
    target=out/f'frame-{i+1:03d}.png'
    if not target.exists():
        source.seek(i)
        frame=remove(source.convert('RGB'))
        frame.save(target)
    print(f'Frame {i+1}/70',flush=True)
contact=Image.new('RGB',(660,248),'#e5e8ee')
for n,i in enumerate([0,14,28,42,56,69]):
    f=Image.open(out/f'frame-{i+1:03d}.png').convert('RGBA')
    contact.paste(f,((n%3)*220,(n//3)*124),f.getchannel('A'))
contact.resize((1320,496),Image.Resampling.NEAREST).save(root/'preview-birefnet.jpg')
