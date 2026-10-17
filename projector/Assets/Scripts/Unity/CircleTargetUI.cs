using UnityEngine;
using Struckout.Domain;

namespace Struckout.Unity
{
    [RequireComponent(typeof(SpriteRenderer))]
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        [SerializeField]
        [Tooltip("移動にかける秒数。0 なら瞬間移動する。")]
        private float _moveDurationSeconds = 0.15f;

        [SerializeField]
        [Tooltip("開始・終了時に的がフェードする秒数。")]
        private float _visibilityDurationSeconds = 0.3f;

        [SerializeField]
        [Tooltip("命中後の移動中にだけ縮む割合。外径と判定の差を小さく抑える。")]
        private float _moveSquash = 0.05f;

        [SerializeField]
        [Tooltip("円の輪郭は動かさず、模様だけがわずかに揺れて見える角度。")]
        private float _rotationAmplitudeDegrees = 1.5f;

        [Header("Hit Ripple")]
        [SerializeField, Min(0.01f)]
        [Tooltip("的の移動元と移動先で波紋が広がる秒数。")]
        private float _rippleDurationSeconds = 0.32f;

        [SerializeField, Min(1f)]
        [Tooltip("波紋リングの最大サイズ。的の直径に対する倍率。")]
        private float _rippleEndScale = 1.3f;

        [SerializeField]
        private Color _rippleColor = new(0.184f, 0.89f, 0.941f, 0.85f);

        private SpriteRenderer _renderer;
        private WorldCoordinateTransform _world;
        private Target _target;
        private Color _baseColor;
        private Vector3 _baseScale = Vector3.one;
        private float _visibility;
        private float _targetVisibility;
        private float _visibilityDelay;
        private float _rotationPhase;

        private Vector3 _moveFrom;
        private Vector3 _moveTo;
        private float _moveElapsed;
        private bool _moving;
        private float _moveSquashAmount;

        private void Awake()
        {
            CacheRenderer();
            if (_renderer != null) _baseColor = _renderer.color;
        }

        private void CacheRenderer()
        {
            if (_renderer == null) _renderer = GetComponent<SpriteRenderer>();

            // Prefab に絵が割り当てられていなければ仮のものを使う。
            // 素材ができたら Prefab 側で差し替えれば、ここは通らなくなる。
            if (_renderer != null && _renderer.sprite == null)
            {
                _renderer.sprite = PlaceholderSprites.Circle;
            }
        }

        public void Initialize(Target target, WorldCoordinateTransform world)
        {
            _world = world;
            _target = target;
            _rotationPhase = target.Coordinate.X * 0.005f + target.Coordinate.Y * 0.011f;
            CacheRenderer();

            transform.localPosition = ToWorld(target);
            ApplyDiameter(target);

            _moving = false;
            _visibility = 0f;
            _targetVisibility = 0f;
            ApplyVisuals();
        }

        public void SetPresentationVisible(bool visible, float delaySeconds = 0f, bool immediate = false)
        {
            _targetVisibility = visible ? 1f : 0f;
            _visibilityDelay = visible ? Mathf.Max(0f, delaySeconds) : 0f;
            if (immediate || _visibilityDurationSeconds <= 0f)
            {
                _visibility = _targetVisibility;
                _visibilityDelay = 0f;
            }
            ApplyVisuals();
        }

        public void MoveTo(Target target)
        {
            _target = target;
            var sourcePosition = transform.localPosition;
            var destination = ToWorld(target);

            // 元の着弾演出を残し、移動元から波紋を広げる。
            SpawnRipple(sourcePosition, startScale: 1f);

            // 大きさは変わらない仕様だが、変わっても破綻しないよう毎回合わせておく。
            ApplyDiameter(target);

            if (_moveDurationSeconds <= 0f)
            {
                transform.localPosition = destination;
                _moving = false;
                _moveSquashAmount = 0f;
                ApplyVisuals();
                SpawnRipple(destination, startScale: 0.08f);
                return;
            }

            _moveFrom = sourcePosition;
            _moveTo = destination;
            _moveElapsed = 0f;
            _moving = true;
        }

        private void Update()
        {
            if (_moving)
            {
                _moveElapsed += Time.deltaTime;
                float t = Mathf.Clamp01(_moveElapsed / _moveDurationSeconds);

                // 端で滑らかに止め、移動中だけ少し縮めて速度を感じさせる。
                transform.localPosition = Vector3.Lerp(_moveFrom, _moveTo, Mathf.SmoothStep(0f, 1f, t));
                _moveSquashAmount = Mathf.Sin(t * Mathf.PI) * Mathf.Clamp01(_moveSquash);
                if (t >= 1f)
                {
                    _moving = false;
                    _moveSquashAmount = 0f;
                    SpawnRipple(_moveTo, startScale: 0.08f);
                }
            }

            if (_visibilityDelay > 0f) _visibilityDelay -= Time.deltaTime;
            else if (!Mathf.Approximately(_visibility, _targetVisibility))
            {
                _visibility = Mathf.MoveTowards(_visibility, _targetVisibility,
                    Time.deltaTime / Mathf.Max(0.001f, _visibilityDurationSeconds));
            }

            ApplyVisuals();
        }

        private void SpawnRipple(Vector3 position, float startScale)
        {
            if (_world == null || _target == null || transform.parent == null) return;

            var sprite = PlaceholderSprites.RippleRing;
            var ripple = new GameObject("TargetRipple", typeof(SpriteRenderer));
            ripple.transform.SetParent(transform.parent, false);
            ripple.transform.localPosition = position;

            var renderer = ripple.GetComponent<SpriteRenderer>();
            renderer.sprite = sprite;
            renderer.color = _rippleColor;
            renderer.sortingOrder = (_renderer != null ? _renderer.sortingOrder : 0) + 1;

            float worldDiameter = _world.ToWorldLength(_target.Diameter);
            float spriteWidth = sprite.bounds.size.x;
            ripple.transform.localScale = spriteWidth > 0f
                ? Vector3.one * (worldDiameter / spriteWidth)
                : Vector3.one;

            var animation = ripple.AddComponent<CollisionMarker>();
            animation.Configure(_rippleDurationSeconds, startScale, _rippleEndScale, 0f);
        }

        private void ApplyVisuals()
        {
            if (_renderer != null)
            {
                var color = _baseColor;
                color.a = _baseColor.a * Mathf.SmoothStep(0f, 1f, _visibility);
                _renderer.color = color;
            }

            transform.localScale = _baseScale * (1f - _moveSquashAmount);
            float rotation = _visibility > 0f
                ? Mathf.Sin(Time.time * 1.8f + _rotationPhase) * _rotationAmplitudeDegrees
                : 0f;
            transform.localRotation = Quaternion.Euler(0f, 0f, rotation);
        }

        /// <summary>
        /// Target.Size は直径。CollisionSolver は Radius (= Size / 2) で判定するので、
        /// 直径をそのまま描画すれば見た目と当たり判定が一致する。
        ///
        /// スプライトの実寸 (bounds) を見てから倍率を出しているのは、
        /// 割り当てられた絵の pixelsPerUnit や余白がいくつでも合うようにするため。
        /// 「1 unit 角の絵が来る」と決め打つと、素材を差し替えた瞬間に
        /// 見た目と当たり判定がずれる。
        /// </summary>
        private void ApplyDiameter(Target target)
        {
            if (_world == null) return;

            float desired = _world.ToWorldLength(target.Diameter);

            float spriteWidth = _renderer != null && _renderer.sprite != null
                ? _renderer.sprite.bounds.size.x
                : 0f;

            if (spriteWidth <= 0f)
            {
                Debug.LogWarning($"{name}: sprite has no width; cannot size {target}.");
                return;
            }

            _baseScale = Vector3.one * (desired / spriteWidth);
            ApplyVisuals();
        }

        private Vector3 ToWorld(Target target) => new(
            _world.ToWorldX(target.Coordinate.X),
            _world.ToWorldY(target.Coordinate.Y),
            0f);
    }
}
