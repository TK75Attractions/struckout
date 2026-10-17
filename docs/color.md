# カラーパレットについて
色空間はHCTを使っている。デザイントークンの考え方を参考にしている。

sys.* はsurfaceやoutlineなど意味をもたせたもの。Semantic Tokenとも言う。
ref.* は sys.* に参照されるもので、primary_70など。Primitive Tokenとも言う。
詳しくは[Material Design](https://m3.material.io/foundations/design-tokens/overview)を参照されたし。

HCTについては[Material Designのドキュメント](https://m3.material.io/styles/color/system/how-the-system-works#e1e92a3b-8702-46b6-8132-58321aa600bd)が詳しい。

Hue(色相)、Chroma(彩度)を固定してToneを10区切りで変えていったのがref/primary_50など。

