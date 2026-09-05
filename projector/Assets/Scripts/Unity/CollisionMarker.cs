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
    /// </summary>
    public class CollisionMarker : MonoBehaviour
    {
        [SerializeField]
        [Tooltip("表示してから消えるまでの秒数。")]
        private float _lifetimeSeconds = 1.0f;

        [SerializeField]
        private float _startScale = 0.6f;

        [SerializeField]
        private float _endScale = 1.6f;

        private CanvasGroup _canvasGroup;
        private float _elapsed;

        private void Awake()
        {
            _canvasGroup = GetComponent<CanvasGroup>();
        }

        private void Update()
        {
            _elapsed += Time.deltaTime;

            float progress = _lifetimeSeconds <= 0f
                ? 1f
                : Mathf.Clamp01(_elapsed / _lifetimeSeconds);

            transform.localScale = Vector3.one * Mathf.Lerp(_startScale, _endScale, progress);

            if (_canvasGroup != null) _canvasGroup.alpha = 1f - progress;

            if (progress >= 1f) Destroy(gameObject);
        }
    }
}
