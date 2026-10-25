using UnityEngine;
using Struckout.Domain;
using System.Collections.Generic;
using System;
using Struckout.Application.Interface;
using Struckout.Domain.TargetDefinition;
using VContainer;
using UnityEngine.UI;
using System.Globalization;
using UnityEngine.Serialization;

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

        private Dictionary<Guid, Transform> _targetToTransform;
        [FormerlySerializedAs("_circleUI")] [SerializeField]
        private Transform circleUI;

        [FormerlySerializedAs("_hudCanvas")]
        [Header("Game HUD")]
        [SerializeField] private Canvas hudCanvas;
        [FormerlySerializedAs("_displayFont")] [SerializeField] private Font displayFont;

        [FormerlySerializedAs("_backgroundSprite")]
        [Header("Projected Screens")]
        [SerializeField] private Sprite backgroundSprite;
        [FormerlySerializedAs("_idleSprite")] [SerializeField] private Sprite idleSprite;
        [FormerlySerializedAs("_finishedSprite")] [SerializeField] private Sprite finishedSprite;
        [FormerlySerializedAs("_screenFadeSeconds")] [SerializeField, Min(0f)] private float screenFadeSeconds = 0.55f;

        [FormerlySerializedAs("_scoredImpactSprite")]
        [Header("Impact Sprites")]
        [SerializeField] private Sprite scoredImpactSprite;
        [FormerlySerializedAs("_missedImpactSprite")] [SerializeField] private Sprite missedImpactSprite;
        [FormerlySerializedAs("_ignoredImpactSprite")] [SerializeField] private Sprite ignoredImpactSprite;
        [FormerlySerializedAs("_scoredImpactSize")] [SerializeField, Min(1f)] private float scoredImpactSize = 280f;
        [FormerlySerializedAs("_missedImpactSize")] [SerializeField, Min(1f)] private float missedImpactSize = 170f;
        [FormerlySerializedAs("_ignoredImpactSize")] [SerializeField, Min(1f)] private float ignoredImpactSize = 150f;

        [FormerlySerializedAs("_collisionMarkerUI")]
        [Header("Collision Marker")]
        [SerializeField]
        [Tooltip("独自の着弾演出 Prefab。未設定なら結果別の画像で演出する。")]
        private Transform collisionMarkerUI;

        [FormerlySerializedAs("_showCollisionMarkers")]
        [SerializeField]
        [Tooltip("off にすると着弾位置を表示しない。")]
        private bool showCollisionMarkers = true;

        [FormerlySerializedAs("_collisionMarkerSize")]
        [SerializeField]
        [Tooltip("マーカーの大きさ。的と同じ盤面ピクセルで指定する。")]
        private float collisionMarkerSize = 48f;

        [FormerlySerializedAs("_hitColor")]
        [SerializeField]
        [Tooltip("的に当たって得点したとき。")]
        private Color hitColor = new(1f, 0.95f, 0f, 0.9f);

        [FormerlySerializedAs("_ignoredColor")]
        [SerializeField]
        [Tooltip("ゲームが始まっていないので判定しなかったとき。")]
        private Color ignoredColor = new(1f, 1f, 1f, 0.9f);

        [FormerlySerializedAs("_missColor")]
        [SerializeField]
        [Tooltip("どの的にも当たらなかったとき。")]
        private Color missColor = new(0f, 0.35f, 1f, 0.9f);

        private UIRoot _uiRoot;
        private WorldCoordinateTransform _world;
        private SpriteRenderer _idleRenderer;
        private SpriteRenderer _finishedRenderer;
        private TextMesh _finishedText;
        private GameObject _hud;
        private Text _scoreText;
        private Text _remainingTimeText;
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

            CreateFieldSprite("ArenaBackground", backgroundSprite, BackgroundSortingOrder);
            _idleRenderer = CreateFieldSprite("IdleScreen", idleSprite, OverlaySortingOrder);
            _finishedRenderer = CreateFieldSprite("FinishedScreen", finishedSprite, OverlaySortingOrder);
            _finishedText = CreateFinishedText();
            CreateHud();
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
            var width = _world.ToWorldLength(_world.Field.width);
            var height = _world.ToWorldLength(_world.Field.height);
            go.transform.localScale = new Vector3(
                width / sprite.bounds.size.x,
                height / sprite.bounds.size.y,
                1f);
            return renderer;
        }

        private TextMesh CreateFinishedText()
        {
            if (finishedSprite == null) return null;

            var go = new GameObject("FinishedMessage", typeof(TextMesh));
            go.transform.SetParent(transform, false);
            var label = go.GetComponent<TextMesh>();
            label.text = "TIME UP";
            label.anchor = TextAnchor.MiddleCenter;
            label.alignment = TextAlignment.Center;
            label.fontSize = 100;
            label.characterSize = 1.2f;
            label.font = displayFont;
            label.color = new Color(1f, 1f, 1f, 0f);
            var textRenderer = go.GetComponent<MeshRenderer>();
            if (textRenderer != null)
            {
                if (displayFont != null) textRenderer.sharedMaterial = displayFont.material;
                textRenderer.sortingOrder = OverlaySortingOrder + 1;
            }
            return label;
        }

        private void CreateHud()
        {
            if (hudCanvas == null || displayFont == null)
            {
                Debug.LogError("[HUD] Assign the HUD Canvas and Oxanium font to UIService.");
                return;
            }

            CreateBuildInfoLabel();

            _hud = new GameObject("GameHud", typeof(RectTransform));
            var root = _hud.GetComponent<RectTransform>();
            root.SetParent(hudCanvas.transform, false);
            root.anchorMin = Vector2.zero;
            root.anchorMax = Vector2.one;
            root.offsetMin = root.offsetMax = Vector2.zero;

            _scoreText = CreateHudLabel("Score", root, false, 800f);
            _remainingTimeText = CreateHudLabel("RemainingTime", root, true, 480f);
            SetScore(0);
            _remainingTimeText.text = "TIME  --:--";
            _hud.SetActive(false);
        }

        private void CreateBuildInfoLabel()
        {
            var go = new GameObject("BuildInfo", typeof(RectTransform), typeof(Text));
            var rect = go.GetComponent<RectTransform>();
            rect.SetParent(hudCanvas.transform, false);
            rect.anchorMin = rect.anchorMax = rect.pivot = new Vector2(1f, 0f);
            rect.anchoredPosition = new Vector2(-24f, 16f);
            rect.sizeDelta = new Vector2(700f, 40f);

            var label = go.GetComponent<Text>();
            label.font = displayFont;
            label.fontSize = 22;
            label.alignment = TextAnchor.MiddleRight;
            label.color = new Color(1f, 1f, 1f, 0.55f);
            label.supportRichText = false;
            label.raycastTarget = false;
            label.text = BuildInfo.Display;
        }

        private Text CreateHudLabel(string objectName, RectTransform root, bool right, float width)
        {
            var panel = new GameObject(objectName, typeof(RectTransform), typeof(Image));
            var rect = panel.GetComponent<RectTransform>();
            rect.SetParent(root, false);
            rect.anchorMin = rect.anchorMax = rect.pivot = new Vector2(right ? 1f : 0f, 1f);
            rect.anchoredPosition = new Vector2(right ? -48f : 48f, -36f);
            rect.sizeDelta = new Vector2(width, 104f);
            var background = panel.GetComponent<Image>();
            background.color = new Color(0.015f, 0.025f, 0.05f, 0.8f);
            background.raycastTarget = false;

            var go = new GameObject("Label", typeof(RectTransform), typeof(Text));
            var labelRect = go.GetComponent<RectTransform>();
            labelRect.SetParent(rect, false);
            labelRect.anchorMin = Vector2.zero;
            labelRect.anchorMax = Vector2.one;
            labelRect.offsetMin = new Vector2(24f, 8f);
            labelRect.offsetMax = new Vector2(-24f, -8f);
            var label = go.GetComponent<Text>();
            label.font = displayFont;
            label.fontSize = 48;
            label.alignment = right ? TextAnchor.MiddleRight : TextAnchor.MiddleLeft;
            label.color = Color.white;
            label.supportRichText = false;
            label.raycastTarget = false;
            return label;
        }

        public void SetRemainingTime(TimeSpan remaining)
        {
            if (_remainingTimeText == null) return;
            var seconds = Math.Max(0L, remaining.Ticks / TimeSpan.TicksPerSecond);
            _remainingTimeText.text = string.Format(CultureInfo.InvariantCulture,
                "TIME  {0:00}:{1:00}", seconds / 60, seconds % 60);
        }

        public void SetScore(uint totalScore)
        {
            if (_scoreText != null)
                _scoreText.text = "SCORE  " + totalScore.ToString(CultureInfo.InvariantCulture);
        }

        private void Update()
        {
            if (_world == null) return;

            var step = screenFadeSeconds <= 0f
                ? 1f
                : Time.deltaTime / screenFadeSeconds;
            _idleOpacity = Mathf.MoveTowards(_idleOpacity, _phase == GamePhase.Idle ? 1f : 0f, step);
            _finishedOpacity = Mathf.MoveTowards(_finishedOpacity, _phase == GamePhase.Finished ? 1f : 0f, step);
            SetAlpha(_idleRenderer, _idleOpacity);
            SetAlpha(_finishedRenderer, _finishedOpacity);

            if (!_finishedText) return;

            var color = _finishedText.color;
            color.a = _finishedOpacity;
            _finishedText.color = color;
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
            if (_hud != null)
            {
                _hud.SetActive(phase != GamePhase.Idle);
                if (phase == GamePhase.Playing)
                {
                    SetScore(0);
                    _remainingTimeText.text = "TIME  --:--";
                }
                else if (phase == GamePhase.Finished)
                {
                    SetRemainingTime(TimeSpan.Zero);
                }
            }
            var index = 0;
            foreach (var trans in _targetToTransform.Values)
            {
                if (trans != null && trans.TryGetComponent<CircleTargetUI>(out var targetUI))
                {
                    targetUI.SetPresentationVisible(phase == GamePhase.Playing, index * 0.07f);
                    index++;
                }
            }
        }

        public void AddTarget(Target target)
        {
            if (target == null) return;
            if (_targetToTransform == null) throw new InvalidOperationException("UIService is not initialized.");
            if (_targetToTransform.ContainsKey(target.Id)) return;

            Transform instantiatedTrans;
            switch (target.Type)
            {
                case TargetType.Circle:
                    if (circleUI == null)
                    {
                        Debug.LogError("CircleUI is null");
                        return;
                    }

                    if (!TryInstantiateTargetUI<CircleTargetUI>(circleUI, out var trans))
                    {
                        Debug.LogError("CircleTargetUI is null");
                        return;
                    }
                    instantiatedTrans = trans;
                    break;
                default:
                    Debug.LogError($"Missing TargetType {target.Type}");
                    return;
            }
            var ui = instantiatedTrans.GetComponent<ITargetUI>() ?? throw new InvalidOperationException("The Prefab doesn't contain ITargetUI");
            ui.Initialize(target, _world);
            _targetToTransform[target.Id] = instantiatedTrans;
            if (instantiatedTrans.TryGetComponent<CircleTargetUI>(out var circle))
            {
                circle.SetPresentationVisible(_phase == GamePhase.Playing,
                    _phase == GamePhase.Playing ? (_targetToTransform.Count - 1) * 0.07f : 0f,
                    _phase != GamePhase.Playing);
            }
        }

        private bool TryInstantiateTargetUI<TTargetUI>(Transform prefab, out Transform trans) where TTargetUI : MonoBehaviour, ITargetUI
        {
            if (prefab.GetComponent<TTargetUI>() == null)
            {
                trans = null;
                Debug.LogError("There are no ITargetUI");
                return false;
            }

            trans = Instantiate(prefab, _uiRoot.TargetRoot);
            return true;
        }

        public void ShowCollisionMarker(float x, float y, CollisionResult result)
        {
            if (!showCollisionMarkers) return;
            if (_uiRoot == null || _uiRoot.TargetRoot == null)
            {
                Debug.LogWarning("UIRoot is not ready; cannot show a collision marker.");
                return;
            }

            var colour = result switch
            {
                CollisionResult.Scored => hitColor,
                CollisionResult.Ignored => ignoredColor,
                _ => missColor,
            };

            var sprite = result switch
            {
                CollisionResult.Scored => scoredImpactSprite,
                CollisionResult.Ignored => ignoredImpactSprite,
                _ => missedImpactSprite,
            };
            var size = result switch
            {
                CollisionResult.Scored => scoredImpactSize,
                CollisionResult.Ignored => ignoredImpactSize,
                _ => missedImpactSize,
            };

            Transform marker;
            if (collisionMarkerUI != null)
            {
                // 差し替えた Prefab は見た目も寿命もその Prefab の責任。
                // こちらから CollisionMarker を足すと、独自のアニメーションを途中で潰しかねない。
                marker = Instantiate(collisionMarkerUI, _uiRoot.TargetRoot);
            }
            else
            {
                marker = CreateSpriteMarker(sprite, sprite == null ? collisionMarkerSize : size,
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
            var desired = _world.ToWorldLength(size);
            var spriteWidth = renderer.sprite.bounds.size.x;
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

        /// <summary>
        /// Removes target from UI.
        /// </summary>
        /// <param name="id">Id of the target to remove.</param>
        /// <returns>Whether target existed and was removed successfully or not.</returns>
        /// <exception cref="InvalidOperationException"></exception>
        public bool RemoveTarget(Guid id)
        {
            if (!_targetToTransform.Remove(id, out var targetTrans)) return false;
            if (targetTrans == null) return false;
            var targetUI = targetTrans.GetComponent<ITargetUI>()
                ?? throw new InvalidOperationException("The target UI has no ITargetUI.");
            targetUI.Remove();
            return true;
        }
    }
}
