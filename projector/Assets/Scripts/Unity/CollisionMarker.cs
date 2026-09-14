using UnityEngine;

namespace Struckout.Unity
{
    /// <summary>
    /// ボールが当たった位置に一瞬だけ出るマーカー。
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
        [SerializeField]
        [Tooltip("表示してから消えるまでの秒数。")]
        private float _lifetimeSeconds = 1.0f;

        [SerializeField]
        private float _startScale = 0.6f;

        [SerializeField]
        private float _endScale = 1.6f;

        private SpriteRenderer _renderer;
        private Vector3 _baseScale = Vector3.one;
        private Color _baseColour = Color.white;
        private float _elapsed;

        private void Awake()
        {
            _renderer = GetComponent<SpriteRenderer>();
            if (_renderer != null) _baseColour = _renderer.color;

            // UIService が的と同じ大きさの基準で置いてくれているので、
            // その値を 1.0 とみなして拡大率をかける。
            _baseScale = transform.localScale;
        }

        private void Update()
        {
            _elapsed += Time.deltaTime;

            float progress = _lifetimeSeconds <= 0f
                ? 1f
                : Mathf.Clamp01(_elapsed / _lifetimeSeconds);

            transform.localScale = _baseScale * Mathf.Lerp(_startScale, _endScale, progress);

            if (_renderer != null)
            {
                var colour = _baseColour;
                colour.a = _baseColour.a * (1f - progress);
                _renderer.color = colour;
            }

            if (progress >= 1f) Destroy(gameObject);
        }
    }
}
