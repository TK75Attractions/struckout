using UnityEngine;
using UnityEngine.UI;
using Struckout.Domain;

namespace Struckout.Unity
{
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        [SerializeField]
        [Tooltip("色を変える対象。未設定なら子から Graphic を探す。")]
        private Graphic _graphic;

        [SerializeField]
        [Tooltip("通常時の色。")]
        private Color _normalColor = Color.white;

        [SerializeField]
        [Tooltip("クールダウン中の色。当たっても得点にならないことを示す。")]
        private Color _cooldownColor = new(0.25f, 0.35f, 0.55f, 1f);

        Target _target;

        private float _cooldownTotal;
        private float _cooldownRemaining;

        private void Awake()
        {
            if (_graphic == null) _graphic = GetComponentInChildren<Graphic>();
            ApplyColor();
        }

        public void Initialize(Target target)
        {
            _target = target;

            RectTransform rect = GetComponent<RectTransform>();
            rect.anchoredPosition = new Vector2(target.Coordinate.X, target.Coordinate.Y);

            // Target.Size は直径。CollisionSolver は Radius (= Size / 2) で判定するので、
            // 直径をそのまま描画すれば見た目と当たり判定が一致する。
            //
            // localScale で大きさを決めると Prefab の sizeDelta が 2 であることに
            // 暗黙に依存してしまうため、sizeDelta を直接指定する。
            rect.sizeDelta = new Vector2(target.Diameter, target.Diameter);
            rect.localScale = Vector3.one;
        }

        public void OnCollision(float cooldownSeconds)
        {
            _cooldownTotal = Mathf.Max(0f, cooldownSeconds);
            _cooldownRemaining = _cooldownTotal;
            ApplyColor();
        }

        private void Update()
        {
            if (_cooldownRemaining <= 0f) return;

            _cooldownRemaining -= Time.deltaTime;
            if (_cooldownRemaining < 0f) _cooldownRemaining = 0f;

            ApplyColor();
        }

        /// <summary>
        /// クールダウン中は色を変える。残り時間に応じて通常色に戻していくので、
        /// あとどれくらいで撃てるようになるかが見て分かる。
        /// </summary>
        private void ApplyColor()
        {
            if (_graphic == null) return;

            if (_cooldownRemaining <= 0f || _cooldownTotal <= 0f)
            {
                _graphic.color = _normalColor;
                return;
            }

            float remaining = _cooldownRemaining / _cooldownTotal;
            _graphic.color = Color.Lerp(_normalColor, _cooldownColor, remaining);
        }
    }
}
