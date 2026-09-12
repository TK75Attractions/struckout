// 的のネオンリング。UV から解析的に描くので、直径 500px でも輪郭が鮮鋭。
//
// **_TargetEdgeUv がこのシェーダと当たり判定をつなぐ唯一の値。**
// リングの外縁が UV 半径のどこに来るかを表し、CircleTargetUI がこれを読んで
// 「外縁 = Target.Radius」になるように拡縮する。0.5 未満にしてある余白は
// 外側へのグローの置き場で、ここが 0.5 だとグローが四角く切れる。
// 値を変えても script 側は追従するので、片方だけ直してずれることはない。
//
// 2D ライトは意図的に受けない。ネオンは自分が光源なので、
// Sprite-Lit にすると Global Light 2D の強度に見た目が振り回される。
//
// スプライトは PlaceholderSprites.Quad を前提にしている (UV が 0..1 の全面)。
// アトラス化されたスプライトを割り当てると UV が 0..1 に収まらず崩れる。
Shader "Struckout/NeonRing"
{
    Properties
    {
        [PerRendererData] _MainTex ("Sprite Texture", 2D) = "white" {}

        _NeonColor ("Neon colour", Color) = (0.184, 0.890, 0.941, 1)

        _TargetEdgeUv ("Target edge (UV radius)", Range(0.2, 0.5)) = 0.42
        _Thickness ("Ring thickness (UV)", Range(0.005, 0.2)) = 0.05
        _Softness ("Edge softness (px)", Range(0.5, 4.0)) = 1.2

        _GlowWidth ("Glow width (UV)", Range(0.0, 0.2)) = 0.07
        _GlowStrength ("Glow strength", Range(0.0, 2.0)) = 0.85
        _CoreFill ("Core fill", Range(0.0, 0.5)) = 0.06

        _RippleWidth ("Ripple width (UV)", Range(0.002, 0.08)) = 0.018
        _RippleSoftness ("Ripple softness", Range(0.5, 4.0)) = 1.5

        // ここから下は CircleTargetUI が的ごとに差し替える (MaterialPropertyBlock)。
        // 休止中は _Phase = 1 / _RippleA = 0 で、何も足さない状態に戻る。
        [HideInInspector] _Phase ("Ring scale (0-1)", Range(0.0, 1.0)) = 1
        [HideInInspector] _RippleR ("Ripple radius (UV)", Range(0.0, 0.5)) = 0
        [HideInInspector] _RippleA ("Ripple alpha", Range(0.0, 2.0)) = 0

        [HideInInspector] _RendererColor ("RendererColor", Color) = (1,1,1,1)
    }

    SubShader
    {
        Tags
        {
            "Queue" = "Transparent"
            "RenderType" = "Transparent"
            "RenderPipeline" = "UniversalPipeline"
            "IgnoreProjector" = "True"
            "PreviewType" = "Plane"
        }

        Cull Off
        ZWrite Off
        // プロジェクタは加算光なので、黒地では加算合成が実物の見え方に近い。
        Blend SrcAlpha One

        Pass
        {
            Tags { "LightMode" = "Universal2D" }

            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            struct Attributes
            {
                float3 positionOS : POSITION;
                float4 color      : COLOR;
                float2 uv         : TEXCOORD0;
            };

            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                float4 color      : COLOR;
                float2 uv         : TEXCOORD0;
            };

            TEXTURE2D(_MainTex);
            SAMPLER(sampler_MainTex);

            CBUFFER_START(UnityPerMaterial)
                float4 _MainTex_ST;
                half4  _NeonColor;
                float  _TargetEdgeUv;
                float  _Thickness;
                float  _Softness;
                float  _GlowWidth;
                float  _GlowStrength;
                float  _CoreFill;
                float  _RippleWidth;
                float  _RippleSoftness;
                float  _Phase;
                float  _RippleR;
                float  _RippleA;
            CBUFFER_END

            Varyings vert (Attributes input)
            {
                Varyings output;
                output.positionCS = TransformObjectToHClip(input.positionOS);
                output.uv = TRANSFORM_TEX(input.uv, _MainTex);
                output.color = input.color;
                return output;
            }

            half4 frag (Varyings input) : SV_Target
            {
                // 中心からの距離。0 が中心、0.5 が四角の縁。
                float d = length(input.uv - 0.5);

                // _Phase はリングの縮尺。1 で通常、0 で中心に畳まれて消える。
                // 太さとグロー幅も一緒に縮めるので、見た目は相似形のまま小さくなる。
                float phase = saturate(_Phase);
                float edge  = _TargetEdgeUv * phase;
                float inner = max(edge - _Thickness * phase, 0.0);

                // 画面上で常に同じ太さのアンチエイリアスにする。
                // これがあるので、拡縮しても縁がぼけたりジャギったりしない。
                float aa = max(fwidth(d), 1e-5) * _Softness;

                float ring = smoothstep(inner - aa, inner + aa, d)
                           * (1.0 - smoothstep(edge - aa, edge + aa, d));

                float glowWidth = max(_GlowWidth * phase, 1e-5);

                // 外向きのグロー。二乗して中心寄りに寄せる。
                float outward = 1.0 - saturate((d - edge) / glowWidth);
                outward = outward * outward * step(edge, d);

                // 内向きのグロー。
                float inward = 1.0 - saturate((inner - d) / glowWidth);
                inward = inward * inward * step(d, inner);

                float glow = (outward + inward) * _GlowStrength;

                // 中心の淡い塗り。的の内側であることが分かる程度。
                float core = (1.0 - smoothstep(inner - aa, inner + aa, d)) * _CoreFill;

                // 波紋。的が消えるとき・現れるときに外へ広がる 1 本のリング。
                // _RippleA が 0 の間は何も足さないので、休止中の見た目は変わらない。
                float rippleAa = max(fwidth(d), 1e-5) * _RippleSoftness;
                float rippleBand = abs(d - _RippleR) - _RippleWidth * 0.5;
                float ripple = (1.0 - smoothstep(-rippleAa, rippleAa, rippleBand)) * _RippleA;

                half4 tint = _NeonColor * input.color;
                float alpha = saturate(ring + glow + core + ripple) * tint.a;

                return half4(tint.rgb, alpha);
            }
            ENDHLSL
        }
    }

    Fallback Off
}
