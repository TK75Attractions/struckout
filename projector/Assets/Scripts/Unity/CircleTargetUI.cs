using UnityEngine;
using Struckout.Domain;

namespace Struckout.Unity
{
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        [SerializeField]
        [Tooltip("移動にかける秒数。0 なら瞬間移動する。")]
        private float _moveDurationSeconds = 0.15f;

        private RectTransform _rect;
        private Target _target;

        private Vector2 _moveFrom;
        private Vector2 _moveTo;
        private float _moveElapsed;
        private bool _moving;

        private void Awake()
        {
            _rect = GetComponent<RectTransform>();
        }

        public void Initialize(Target target)
        {
            _target = target;
            if (_rect == null) _rect = GetComponent<RectTransform>();

            _rect.anchoredPosition = ToAnchored(target);

            // Target.Size は直径。CollisionSolver は Radius (= Size / 2) で判定するので、
            // 直径をそのまま描画すれば見た目と当たり判定が一致する。
            //
            // localScale で大きさを決めると Prefab の sizeDelta が 2 であることに
            // 暗黙に依存してしまうため、sizeDelta を直接指定する。
            _rect.sizeDelta = new Vector2(target.Diameter, target.Diameter);
            _rect.localScale = Vector3.one;

            _moving = false;
        }

        public void MoveTo(Target target)
        {
            _target = target;

            if (_moveDurationSeconds <= 0f)
            {
                _rect.anchoredPosition = ToAnchored(target);
                _moving = false;
                return;
            }

            _moveFrom = _rect.anchoredPosition;
            _moveTo = ToAnchored(target);
            _moveElapsed = 0f;
            _moving = true;
        }

        private void Update()
        {
            if (!_moving) return;

            _moveElapsed += Time.deltaTime;
            float t = Mathf.Clamp01(_moveElapsed / _moveDurationSeconds);

            // 端で滑らかに止まるほうが的として見やすい。
            _rect.anchoredPosition = Vector2.Lerp(_moveFrom, _moveTo, Mathf.SmoothStep(0f, 1f, t));

            if (t >= 1f) _moving = false;
        }

        private static Vector2 ToAnchored(Target target) =>
            new(target.Coordinate.X, target.Coordinate.Y);
    }
}
