using UnityEngine;
using Struckout.Domain;
using Struckout.Application;
using System.Collections.Generic;
using System;
using VContainer;

namespace Struckout.Unity
{
    /// <summary>
    /// 盤面の描画。的とマーカーをスプライトとして置く。
    ///
    /// 以前は Screen Space - Overlay の Canvas に uGUI の Image を並べていたが、
    /// Overlay はカメラを介さないため URP 2D (2D ライト、ポストプロセス、
    /// パーティクル) がまったく効かなかった。スプライトに移して描画経路を
    /// カメラに通している。得点表示などの HUD は引き続き Canvas 側の担当。
    ///
    /// 座標は盤面ピクセル (0〜1920, 0〜1080) のまま受け取り、ここと
    /// <see cref="CircleTargetUI"/> でワールド座標に直す。GameRuntime から上と
    /// TargetGenerator は、この移行の影響を受けない。
    /// </summary>
    public class UIService : MonoBehaviour, IUIService
    {
        private const int BackgroundSortingOrder = -100;
        private const int OverlaySortingOrder = 100;
        private const int MarkerSortingOrder = 150;

        private Dictionary<Target, Transform> _targetToTransform;
        [SerializeField]
        private Transform _circleUI;

        [Header("Projected Screens")]
        [SerializeField] private Sprite _backgroundSprite;
        [SerializeField] private Sprite _idleSprite;
        [SerializeField] private Sprite _finishedSprite;
        [SerializeField, Min(0f)] private float _screenFadeSeconds = 0.55f;

        [Header("Impact Sprites")]
        [SerializeField] private Sprite _scoredImpactSprite;
        [SerializeField] private Sprite _missedImpactSprite;
        [SerializeField] private Sprite _ignoredImpactSprite;
        [SerializeField, Min(1f)] private float _scoredImpactSize = 280f;
        [SerializeField, Min(1f)] private float _missedImpactSize = 170f;
        [SerializeField, Min(1f)] private float _ignoredImpactSize = 150f;

        [Header("Collision Marker")]
        [SerializeField]
        [Tooltip("独自の着弾演出 Prefab。未設定なら結果別の画像で演出する。")]
        private Transform _collisionMarkerUI;

        [SerializeField]
        [Tooltip("off にすると着弾位置を表示しない。")]
        private bool _showCollisionMarkers = true;

        [SerializeField]
        [Tooltip("マーカーの大きさ。的と同じ盤面ピクセルで指定する。")]
        private float _collisionMarkerSize = 48f;

        [SerializeField]
        [Tooltip("的に当たって得点したとき。")]
        private Color _hitColor = new(1f, 0.95f, 0f, 0.9f);

        [SerializeField]
        [Tooltip("ゲームが始まっていないので判定しなかったとき。")]
        private Color _ignoredColor = new(1f, 1f, 1f, 0.9f);

        [SerializeField]
        [Tooltip("どの的にも当たらなかったとき。")]
        private Color _missColor = new(0f, 0.35f, 1f, 0.9f);

        private UIRoot _uiRoot;
        private WorldCoordinateTransform _world;
        private SpriteRenderer _idleRenderer;
        private SpriteRenderer _finishedRenderer;
        private TextMesh _finishedText;
        private GamePhase _phase = GamePhase.Idle;
        private float _idleOpacity = 1f;
        private float _finishedOpacity;

        [Inject]
        public void Construct(
            UIRoot uiRoot,
            WorldCoordinateTransform world
        )
        {
            _uiRoot = uiRoot;
            _world = world;
            _targetToTransform = new();

            CreateFieldSprite("ArenaBackground", _backgroundSprite, BackgroundSortingOrder);
            _idleRenderer = CreateFieldSprite("IdleScreen", _idleSprite, OverlaySortingOrder);
            _finishedRenderer = CreateFieldSprite("FinishedScreen", _finishedSprite, OverlaySortingOrder);
            _finishedText = CreateFinishedText();
            SetAlpha(_idleRenderer, _idleOpacity);
            SetAlpha(_finishedRenderer, _finishedOpacity);
        }

        private SpriteRenderer CreateFieldSprite(string objectName, Sprite sprite, int sortingOrder)
        {
            if (sprite == null) return null;

            var go = new GameObject(objectName, typeof(SpriteRenderer));
            go.transform.SetParent(transform, false);
            var renderer = go.GetComponent<SpriteRenderer>();
            renderer.sprite = sprite;
            renderer.sortingOrder = sortingOrder;

            // 画面比率に関係なく、論理盤面と同じ矩形を覆う。
            float width = _world.ToWorldLength(_world.Field.Width);
            float height = _world.ToWorldLength(_world.Field.Height);
            go.transform.localScale = new Vector3(
                width / sprite.bounds.size.x,
                height / sprite.bounds.size.y,
                1f);
            return renderer;
        }

        private TextMesh CreateFinishedText()
        {
            if (_finishedSprite == null) return null;

            var go = new GameObject("FinishedMessage", typeof(TextMesh));
            go.transform.SetParent(transform, false);
            var label = go.GetComponent<TextMesh>();
            label.text = "TIME UP";
            label.anchor = TextAnchor.MiddleCenter;
            label.alignment = TextAlignment.Center;
            label.fontSize = 100;
            label.characterSize = 1.2f;
            label.color = new Color(1f, 1f, 1f, 0f);
            var textRenderer = go.GetComponent<MeshRenderer>();
            if (textRenderer != null) textRenderer.sortingOrder = OverlaySortingOrder + 1;
            return label;
        }

        private void Update()
        {
            if (_world == null) return;

            float step = _screenFadeSeconds <= 0f
                ? 1f
                : Time.deltaTime / _screenFadeSeconds;
            _idleOpacity = Mathf.MoveTowards(_idleOpacity, _phase == GamePhase.Idle ? 1f : 0f, step);
            _finishedOpacity = Mathf.MoveTowards(_finishedOpacity, _phase == GamePhase.Finished ? 1f : 0f, step);
            SetAlpha(_idleRenderer, _idleOpacity);
            SetAlpha(_finishedRenderer, _finishedOpacity);

            if (_finishedText != null)
            {
                var color = _finishedText.color;
                color.a = _finishedOpacity;
                _finishedText.color = color;
            }
        }

        private static void SetAlpha(SpriteRenderer renderer, float alpha)
        {
            if (renderer == null) return;
            var color = renderer.color;
            color.a = alpha;
            renderer.color = color;
        }

        public void SetGamePhase(GamePhase phase)
        {
            _phase = phase;
            int index = 0;
            foreach (var transform in _targetToTransform.Values)
            {
                if (transform != null && transform.TryGetComponent<CircleTargetUI>(out var targetUI))
                {
                    targetUI.SetPresentationVisible(phase == GamePhase.Playing, index * 0.07f);
                    index++;
                }
            }
        }
        
        public void InstantiateTargets(IReadOnlyList<Target> targets)
        {
            foreach (var target in targets)
            {
                if (target == null) Debug.Log("Target is null");
                InstantiateTarget(target);
            }
        }

        public void InstantiateTarget(Target target)
        {
            Transform trans;
            if (target == null) return;
            if (_targetToTransform == null) throw new InvalidOperationException("UIService is not initialized.");
            if (_targetToTransform.TryGetValue(target, out var _)) return;

            switch (target.Type)
            {
                case TargetType.Circle:
                    if (_circleUI == null)
                    {
                        Debug.LogError("CircleUI is null");
                        return;
                    }

                    if(!TryInstantiateTargetUI<CircleTargetUI>(_circleUI, out var transform))
                    {
                        Debug.LogError("CircleTargetUI is null");
                        return;
                    }
                    trans = transform;
                    break;
                default:
                    Debug.LogError($"Missing TargetType { target.Type }");
                    return;
            }
            var ui = trans.GetComponent<ITargetUI>() ?? throw new Exception("The PrefabDoesn't Contain ITargetUI");
            ui.Initialize(target, _world);
            _targetToTransform[target] = trans;
            if (trans.TryGetComponent<CircleTargetUI>(out var circle))
            {
                circle.SetPresentationVisible(_phase == GamePhase.Playing,
                    _phase == GamePhase.Playing ? (_targetToTransform.Count - 1) * 0.07f : 0f,
                    _phase != GamePhase.Playing);
            }
        }

        bool TryInstantiateTargetUI<TTargetUI>(Transform prefab, out Transform transform) where TTargetUI : MonoBehaviour, ITargetUI
        {
            if(prefab.GetComponent<TTargetUI>() == null)
            {
                transform = null;
                Debug.LogError("There are no ITargetUI");
                return false;
            }
            
            transform = Instantiate(prefab,_uiRoot.TargetRoot);
            return true;
        }

        public void ShowCollisionMarker(float x, float y, CollisionResult result)
        {
            if (!_showCollisionMarkers) return;
            if (_uiRoot == null || _uiRoot.TargetRoot == null)
            {
                Debug.LogWarning("UIRoot is not ready; cannot show a collision marker.");
                return;
            }

            var colour = result switch
            {
                CollisionResult.Scored => _hitColor,
                CollisionResult.Ignored => _ignoredColor,
                _ => _missColor,
            };

            var sprite = result switch
            {
                CollisionResult.Scored => _scoredImpactSprite,
                CollisionResult.Ignored => _ignoredImpactSprite,
                _ => _missedImpactSprite,
            };
            float size = result switch
            {
                CollisionResult.Scored => _scoredImpactSize,
                CollisionResult.Ignored => _ignoredImpactSize,
                _ => _missedImpactSize,
            };

            Transform marker;
            if (_collisionMarkerUI != null)
            {
                // 差し替えた Prefab は見た目も寿命もその Prefab の責任。
                // こちらから CollisionMarker を足すと、独自のアニメーションを途中で潰しかねない。
                marker = Instantiate(_collisionMarkerUI, _uiRoot.TargetRoot);
            }
            else
            {
                marker = CreateSpriteMarker(sprite, sprite == null ? _collisionMarkerSize : size,
                    sprite == null ? colour : Color.white, result);
            }

            // 的と同じ盤面座標で置く。
            marker.localPosition = new Vector3(_world.ToWorldX(x), _world.ToWorldY(y), 0f);
        }

        /// <summary>
        /// 結果別の着弾画像でマーカーを生成する。
        /// 画像未設定時はプレースホルダーの菱形を使用する。
        /// </summary>
        private Transform CreateSpriteMarker(Sprite sprite, float size, Color colour, CollisionResult result)
        {
            var go = new GameObject("CollisionMarker", typeof(SpriteRenderer));
            go.transform.SetParent(_uiRoot.TargetRoot, false);

            var renderer = go.GetComponent<SpriteRenderer>();
            renderer.sprite = sprite != null ? sprite : PlaceholderSprites.Diamond;
            renderer.color = colour;
            renderer.sortingOrder = MarkerSortingOrder;

            // 的と同じ理屈で、スプライトの実寸から倍率を出す。
            float desired = _world.ToWorldLength(size);
            float spriteWidth = renderer.sprite.bounds.size.x;
            go.transform.localScale = spriteWidth > 0f
                ? Vector3.one * (desired / spriteWidth)
                : Vector3.one;

            var animation = go.AddComponent<CollisionMarker>();
            switch (result)
            {
                case CollisionResult.Scored:
                    animation.Configure(0.7f, 0.45f, 1.2f, 14f);
                    break;
                case CollisionResult.Ignored:
                    animation.Configure(0.45f, 0.8f, 1.05f, 0f);
                    break;
                default:
                    animation.Configure(0.55f, 0.7f, 1.12f, 0f);
                    break;
            }

            return go.transform;
        }

        public void MoveTarget(Target target)
        {
            if (!_targetToTransform.TryGetValue(target, out var transform))
            {
                Debug.LogWarning(
                    $"No UI for {target}. UI count={_targetToTransform.Count}");
                return;
            }

            if (transform == null)
            {
                Debug.LogWarning($"The UI for {target} has been destroyed unexpectedly.");
                _targetToTransform.Remove(target);
                return;
            }

            try
            {
                ITargetUI targetui = transform.GetComponent<ITargetUI>();
                if (targetui == null)
                {
                    Debug.LogError("The target UI has no ITargetUI.");
                    return;
                }

                // 的は消さない。同じ GameObject を新しい座標へ移すだけ。
                // Target は同一性で扱うので、辞書の対応づけはそのままでよい。
                targetui.MoveTo(target);
            }
            catch (Exception ex)
            {
                Debug.LogError(ex);
            }
        }
    }
}
