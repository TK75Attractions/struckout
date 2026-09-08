using UnityEngine;
using UnityEngine.UI;
using Struckout.Domain;
using Struckout.Application;
using System.Collections.Generic;
using System;
using VContainer;

namespace Struckout.Unity
{
    public class UIService : MonoBehaviour, IUIService
    {
        private Dictionary<Target, Transform> _targetToTransform;
        [SerializeField]
        private Transform _circleUI;

        [Header("Collision Marker")]
        [SerializeField]
        [Tooltip("当たった位置に出すマーカー。未設定ならコード側で簡易的なものを作る。演出を作ったらここに差し替える。")]
        private RectTransform _collisionMarkerUI;

        [SerializeField]
        [Tooltip("off にすると着弾位置を表示しない。")]
        private bool _showCollisionMarkers = true;

        [SerializeField]
        private float _collisionMarkerSize = 48f;

        [SerializeField]
        [Tooltip("的に当たって得点したとき。")]
        private Color _hitColor = new(0.35f, 1f, 0.45f, 0.9f);

        [SerializeField]
        [Tooltip("的には当たったが、クールダウン中で得点にならなかったとき。")]
        private Color _coolingDownColor = new(1f, 0.9f, 0.25f, 0.9f);

        [SerializeField]
        [Tooltip("どの的にも当たらなかったとき。")]
        private Color _missColor = new(1f, 0.45f, 0.3f, 0.9f);

        private UIRoot _uiRoot;

        [Inject]
        public void Construct(
            UIRoot uiRoot
        )
        {
            _uiRoot = uiRoot;
            _targetToTransform = new();
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
            if (target == null) Debug.Log("Target is null");
            if (_targetToTransform == null) Debug.Log("Dictionary is null");
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
            ui.Initialize(target);
            _targetToTransform[target] = trans;
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
                CollisionResult.CoolingDown => _coolingDownColor,
                _ => _missColor,
            };

            RectTransform marker;
            if (_collisionMarkerUI != null)
            {
                // 差し替えた Prefab は見た目も寿命もその Prefab の責任。
                // こちらから CollisionMarker を足すと、独自のアニメーションを途中で潰しかねない。
                marker = Instantiate(_collisionMarkerUI, _uiRoot.TargetRoot);
            }
            else
            {
                marker = CreateDefaultMarker(colour);
                marker.gameObject.AddComponent<CollisionMarker>();
            }

            // 的と同じ座標系で置く (CircleTarget prefab と同じくアンカーは左下)。
            marker.anchorMin = Vector2.zero;
            marker.anchorMax = Vector2.zero;
            marker.pivot = new Vector2(0.5f, 0.5f);
            marker.anchoredPosition = new Vector2(x, y);
        }

        /// <summary>
        /// Prefab が用意されていないときの最低限のマーカー。
        /// 的が円なので、区別できるよう 45 度回した四角にしている。
        /// </summary>
        private RectTransform CreateDefaultMarker(Color colour)
        {
            var go = new GameObject("CollisionMarker", typeof(RectTransform), typeof(CanvasGroup), typeof(Image));

            var rect = go.GetComponent<RectTransform>();
            rect.SetParent(_uiRoot.TargetRoot, false);
            rect.sizeDelta = new Vector2(_collisionMarkerSize, _collisionMarkerSize);
            rect.localRotation = Quaternion.Euler(0f, 0f, 45f);

            var image = go.GetComponent<Image>();
            image.color = colour;
            image.raycastTarget = false;

            return rect;
        }

        public void OnTargetHit(Target target, float cooldownSeconds)
        {
            if (!_targetToTransform.TryGetValue(target, out var transform))
            {
                Debug.LogWarning(
                    $"No UI for the target at ({target.Coordinate.X:F1}, {target.Coordinate.Y:F1}). " +
                    $"UI count={_targetToTransform.Count}");
                return;
            }

            if (transform == null)
            {
                Debug.LogWarning("The target UI has been destroyed unexpectedly.");
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

                // 的は消さない。クールダウンの見た目にするだけ。
                targetui.OnCollision(cooldownSeconds);
            }
            catch (Exception ex)
            {
                Debug.LogError(ex);
            }
        }
    }
}