"""Rigenera assets/face/yunet_320.onnx dal modello YuNet originale (OpenCV Zoo, licenza MIT).

Il modello originale ha forma fissa 640x640. Qui si porta a 320x320 (circa 4 volte piu veloce
su CPU) togliendo le forme intermedie annotate: le Reshape usano -1 e i Resize scale fisse,
quindi il grafo resta valido per qualsiasi lato multiplo di 32.

Uso: python scripts/prepare_face_model.py face_detection_yunet_2023mar.onnx assets/face/yunet_320.onnx
L'originale: https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet (git LFS)
"""
import sys
import onnx

SIDE = 320
src, dst = sys.argv[1], sys.argv[2]
m = onnx.load(src)
del m.graph.value_info[:]
dims = m.graph.input[0].type.tensor_type.shape.dim
dims[2].dim_value = SIDE
dims[3].dim_value = SIDE
for out in m.graph.output:
    for d in out.type.tensor_type.shape.dim:
        d.Clear()
onnx.save(m, dst)
print("scritto", dst)
