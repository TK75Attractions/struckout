using UnityEngine;
using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using UnityEngine.Serialization;

namespace Struckout.Unity
{
    [RequireComponent(typeof(SpriteRenderer))]
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        [FormerlySerializedAs("_visibilityDurationSeconds")]
        [SerializeField]
        [Tooltip("登録済みの的がフェードインする秒数。")]
        private float visibilityDurationSeconds = 0.3f;

        [FormerlySerializedAs("_rotationAmplitudeDegrees")]
        [SerializeField]
        [Tooltip("円の輪郭は動かさず、模様だけがわずかに揺れて見える角度。")]
        private float rotationAmplitudeDegrees = 1.5f;

        [FormerlySerializedAs("_rippleDurationSeconds")]
        [Header("Hit Ripple")]
        [SerializeField, Min(0.01f)]
        [Tooltip("的の削除位置と新しい的の生成位置で波紋が広がる秒数。")]
        private float rippleDurationSeconds = 0.32f;

        [FormerlySerializedAs("_rippleEndScale")]
        [SerializeField, Min(1f)]
        [Tooltip("波紋リングの最大サイズ。的の直径に対する倍率。")]
        private float rippleEndScale = 1.3f;

        [FormerlySerializedAs("_rippleColor")] [SerializeField]
        private Color rippleColor = new(0.184f, 0.89f, 0.941f, 0.85f);

        private SpriteRenderer _renderer;
        private WorldCoordinateTransform _world;
        private Target _target;
        private Color _baseColor;
        private Vector3 _baseScale = Vector3.one;
        private float _visibility;
        private float _targetVisibility;
        private float _visibilityDelay;
        private float _rotationPhase;

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

            _visibility = 0f;
            _targetVisibility = 0f;
            ApplyVisuals();
            SpawnRipple(transform.localPosition, startScale: 0.08f);
        }

        public void SetPresentationVisible(bool visible, float delaySeconds = 0f, bool immediate = false)
        {
            _targetVisibility = visible ? 1f : 0f;
            _visibilityDelay = visible ? Mathf.Max(0f, delaySeconds) : 0f;
            if (immediate || visibilityDurationSeconds <= 0f)
            {
                _visibility = _targetVisibility;
                _visibilityDelay = 0f;
            }
            ApplyVisuals();
        }

        public void Remove()
        {
            SpawnRipple(transform.localPosition, startScale: 1f);
            Destroy(gameObject);
        }

        private void Update()
        {
            if (_visibilityDelay > 0f) _visibilityDelay -= Time.deltaTime;
            else if (!Mathf.Approximately(_visibility, _targetVisibility))
            {
                _visibility = Mathf.MoveTowards(_visibility, _targetVisibility,
                    Time.deltaTime / Mathf.Max(0.001f, visibilityDurationSeconds));
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
            renderer.color = rippleColor;
            renderer.sortingOrder = (_renderer != null ? _renderer.sortingOrder : 0) + 1;

            var worldDiameter = _world.ToWorldLength(_target.Diameter);
            var spriteWidth = sprite.bounds.size.x;
            ripple.transform.localScale = spriteWidth > 0f
                ? Vector3.one * (worldDiameter / spriteWidth)
                : Vector3.one;

            var animation = ripple.AddComponent<CollisionMarker>();
            animation.Configure(rippleDurationSeconds, startScale, rippleEndScale, 0f);
        }

        private void ApplyVisuals()
        {
            if (_renderer != null)
            {
                var color = _baseColor;
                color.a = _baseColor.a * Mathf.SmoothStep(0f, 1f, _visibility);
                _renderer.color = color;
            }

            transform.localScale = _baseScale;
            var rotation = _visibility > 0f
                ? Mathf.Sin(Time.time * 1.8f + _rotationPhase) * rotationAmplitudeDegrees
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

            var desired = _world.ToWorldLength(target.Diameter);

            var spriteWidth = _renderer != null && _renderer.sprite != null
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
