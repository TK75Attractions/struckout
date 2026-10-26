using UnityEngine;
using UnityEngine.Serialization;

namespace Struckout.Unity
{
    /// <summary>
    /// 着弾位置や的の移動元・移動先に短時間表示する拡大フェード演出。
    ///
    /// デバッグでは「どこに当たったか」を目で確認するために使う。
    /// 変換係数を合わせ込むときは、この位置と的の位置を見比べる。
    ///
    /// 演出を差し替えるときは、この挙動を持った Prefab を作って
    /// <see cref="UIService"/> の Collision Marker UI に割り当てればよい。
    /// その場合このスクリプトは不要で、好きなアニメーションに置き換えられる。
    /// スプライトになったので、パーティクルや 2D ライトを持たせることもできる。
    /// </summary>
    [RequireComponent(typeof(SpriteRenderer))]
    public class CollisionMarker : MonoBehaviour
    {
        [FormerlySerializedAs("_lifetimeSeconds")]
        [SerializeField]
        [Tooltip("表示してから消えるまでの秒数。")]
        private float lifetimeSeconds = 1.0f;

        [FormerlySerializedAs("_startScale")] [SerializeField]
        private float startScale = 0.6f;

        [FormerlySerializedAs("_endScale")] [SerializeField]
        private float endScale = 1.6f;

        private SpriteRenderer _renderer;
        private Vector3 _baseScale = Vector3.one;
        private Color _baseColour = Color.white;
        private float _elapsed;
        private float _rotationDegrees;

        private void Awake()
        {
            _renderer = GetComponent<SpriteRenderer>();
            if (_renderer != null) _baseColour = _renderer.color;

            // UIService が的と同じ大きさの基準で置いてくれているので、
            // その値を 1.0 とみなして拡大率をかける。
            _baseScale = transform.localScale;
        }

        public void Configure(float lifetimeSeconds, float startScale, float endScale, float rotationDegrees)
        {
            this.lifetimeSeconds = lifetimeSeconds;
            this.startScale = startScale;
            this.endScale = endScale;
            _rotationDegrees = rotationDegrees;
            _baseScale = transform.localScale;
            if (_renderer == null) _renderer = GetComponent<SpriteRenderer>();
            if (_renderer != null) _baseColour = _renderer.color;
            _elapsed = 0f;
            Apply(0f);
        }

        private void Update()
        {
            _elapsed += Time.deltaTime;

            var progress = lifetimeSeconds <= 0f
                ? 1f
                : Mathf.Clamp01(_elapsed / lifetimeSeconds);

            Apply(progress);

            if (progress >= 1f) Destroy(gameObject);
        }

        private void Apply(float progress)
        {
            var eased = 1f - (1f - progress) * (1f - progress);
            transform.localScale = _baseScale * Mathf.Lerp(startScale, endScale, eased);
            transform.localRotation = Quaternion.Euler(0f, 0f, _rotationDegrees * eased);

            if (_renderer != null)
            {
                var colour = _baseColour;
                colour.a = _baseColour.a * (1f - progress);
                _renderer.color = colour;
            }

        }
    }
}
