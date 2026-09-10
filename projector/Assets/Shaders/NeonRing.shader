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

                float edge  = _TargetEdgeUv;
                float inner = max(edge - _Thickness, 0.0);

                // 画面上で常に同じ太さのアンチエイリアスにする。
                // これがあるので、拡縮しても縁がぼけたりジャギったりしない。
                float aa = max(fwidth(d), 1e-5) * _Softness;

                float ring = smoothstep(inner - aa, inner + aa, d)
                           * (1.0 - smoothstep(edge - aa, edge + aa, d));

                // 外向きのグロー。二乗して中心寄りに寄せる。
                float outward = 1.0 - saturate((d - edge) / max(_GlowWidth, 1e-5));
                outward = outward * outward * step(edge, d);

                // 内向きのグロー。
                float inward = 1.0 - saturate((inner - d) / max(_GlowWidth, 1e-5));
                inward = inward * inward * step(d, inner);

                float glow = (outward + inward) * _GlowStrength;

                // 中心の淡い塗り。的の内側であることが分かる程度。
                float core = (1.0 - smoothstep(inner - aa, inner + aa, d)) * _CoreFill;

                half4 tint = _NeonColor * input.color;
                float alpha = saturate(ring + glow + core) * tint.a;

                return half4(tint.rgb, alpha);
            }
            ENDHLSL
        }
    }

    Fallback Off
}
