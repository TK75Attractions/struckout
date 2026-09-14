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
        /// <summary>マーカーは的より手前に出す。</summary>
        private const int MarkerSortingOrder = 10;

        private Dictionary<Target, Transform> _targetToTransform;
        [SerializeField]
        private Transform _circleUI;

        [Header("Collision Marker")]
        [SerializeField]
        [Tooltip("当たった位置に出すマーカー。未設定ならコード側で簡易的なものを作る。演出を作ったらここに差し替える。")]
        private Transform _collisionMarkerUI;

        [SerializeField]
        [Tooltip("off にすると着弾位置を表示しない。")]
        private bool _showCollisionMarkers = true;

        [SerializeField]
        [Tooltip("マーカーの大きさ。的と同じ盤面ピクセルで指定する。")]
        private float _collisionMarkerSize = 48f;

        [SerializeField]
        [Tooltip("的に当たって得点したとき。")]
        private Color _hitColor = new(0.35f, 1f, 0.45f, 0.9f);

        [SerializeField]
        [Tooltip("ゲームが始まっていないので判定しなかったとき。")]
        private Color _ignoredColor = new(1f, 0.9f, 0.25f, 0.9f);

        [SerializeField]
        [Tooltip("どの的にも当たらなかったとき。")]
        private Color _missColor = new(1f, 0.45f, 0.3f, 0.9f);

        private UIRoot _uiRoot;
        private WorldCoordinateTransform _world;

        [Inject]
        public void Construct(
            UIRoot uiRoot,
            WorldCoordinateTransform world
        )
        {
            _uiRoot = uiRoot;
            _world = world;
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
            ui.Initialize(target, _world);
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
                CollisionResult.Ignored => _ignoredColor,
                _ => _missColor,
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
                marker = CreateDefaultMarker(colour);
                marker.gameObject.AddComponent<CollisionMarker>();
            }

            // 的と同じ盤面座標で置く。
            marker.localPosition = new Vector3(_world.ToWorldX(x), _world.ToWorldY(y), 0f);
        }

        /// <summary>
        /// Prefab が用意されていないときの最低限のマーカー。
        /// 的が円なので、区別できるよう菱形にしている。
        /// </summary>
        private Transform CreateDefaultMarker(Color colour)
        {
            var go = new GameObject("CollisionMarker", typeof(SpriteRenderer));
            go.transform.SetParent(_uiRoot.TargetRoot, false);

            var renderer = go.GetComponent<SpriteRenderer>();
            renderer.sprite = PlaceholderSprites.Diamond;
            renderer.color = colour;
            renderer.sortingOrder = MarkerSortingOrder;

            // 的と同じ理屈で、スプライトの実寸から倍率を出す。
            float desired = _world.ToWorldLength(_collisionMarkerSize);
            float spriteWidth = renderer.sprite.bounds.size.x;
            go.transform.localScale = spriteWidth > 0f
                ? Vector3.one * (desired / spriteWidth)
                : Vector3.one;

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
