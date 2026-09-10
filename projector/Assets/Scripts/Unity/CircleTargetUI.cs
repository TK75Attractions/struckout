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

        /// <summary>NeonRing シェーダが持つ、的の境界を表す UV 半径。</summary>
        private static readonly int TargetEdgeUv = Shader.PropertyToID("_TargetEdgeUv");

        private SpriteRenderer _renderer;
        private WorldCoordinateTransform _world;
        private Target _target;

        private Vector3 _moveFrom;
        private Vector3 _moveTo;
        private float _moveElapsed;
        private bool _moving;

        private void Awake()
        {
            CacheRenderer();
        }

        private void CacheRenderer()
        {
            if (_renderer == null) _renderer = GetComponent<SpriteRenderer>();

            if (_renderer == null) return;

            // 絵が割り当てられていなければ用意する。
            // 形をシェーダが描くマテリアル (NeonRing) のときは下敷きの四角でよく、
            // そうでなければ仮の円を置く。素材ができたら Prefab 側で差し替える。
            if (_renderer.sprite == null)
            {
                _renderer.sprite = DrawsItsOwnShape
                    ? PlaceholderSprites.Quad
                    : PlaceholderSprites.Circle;
            }
        }

        public void Initialize(Target target, WorldCoordinateTransform world)
        {
            _world = world;
            _target = target;
            CacheRenderer();

            transform.localPosition = ToWorld(target);
            ApplyDiameter(target);

            _moving = false;
        }

        public void MoveTo(Target target)
        {
            _target = target;

            // 大きさは変わらない仕様だが、変わっても破綻しないよう毎回合わせておく。
            ApplyDiameter(target);

            if (_moveDurationSeconds <= 0f)
            {
                transform.localPosition = ToWorld(target);
                _moving = false;
                return;
            }

            _moveFrom = transform.localPosition;
            _moveTo = ToWorld(target);
            _moveElapsed = 0f;
            _moving = true;
        }

        private void Update()
        {
            if (!_moving) return;

            _moveElapsed += Time.deltaTime;
            float t = Mathf.Clamp01(_moveElapsed / _moveDurationSeconds);

            // 端で滑らかに止まるほうが的として見やすい。
            transform.localPosition = Vector3.Lerp(_moveFrom, _moveTo, Mathf.SmoothStep(0f, 1f, t));

            if (t >= 1f) _moving = false;
        }

        /// <summary>
        /// マテリアルが「的の境界が UV 半径のどこか」を宣言しているか。
        /// 宣言していれば形はシェーダが描いている。
        /// </summary>
        private bool DrawsItsOwnShape =>
            _renderer != null
            && _renderer.sharedMaterial != null
            && _renderer.sharedMaterial.HasFloat(TargetEdgeUv);

        /// <summary>
        /// Target.Size は直径。CollisionSolver は Radius (= Size / 2) で判定するので、
        /// 的の境界が直径に一致していれば見た目と当たり判定が一致する。
        ///
        /// スプライトの実寸 (bounds) を見てから倍率を出しているのは、
        /// 割り当てられた絵の pixelsPerUnit や余白がいくつでも合うようにするため。
        /// 「1 unit 角の絵が来る」と決め打つと、素材を差し替えた瞬間にずれる。
        ///
        /// さらに、形をシェーダが描く場合は絵の縁と的の境界が一致しない
        /// (外側にグローを置く余白があるため)。境界がどこかはマテリアルだけが
        /// 知っているので、ここで問い合わせる。値を持たないマテリアルなら
        /// 絵の全幅を的とみなす (従来どおり)。
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

            // 絵の幅のうち、的の直径にあたる割合。
            float visibleFraction = 1f;
            if (DrawsItsOwnShape)
            {
                float edgeUv = _renderer.sharedMaterial.GetFloat(TargetEdgeUv);
                if (edgeUv > 0f) visibleFraction = edgeUv * 2f;
            }

            transform.localScale = Vector3.one * (desired / (spriteWidth * visibleFraction));
        }

        private Vector3 ToWorld(Target target) => new(
            _world.ToWorldX(target.Coordinate.X),
            _world.ToWorldY(target.Coordinate.Y),
            0f);
    }
}
