using UnityEngine;
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

        public void OnCollisionTarget(Target target)
        {
            if (!_targetToTransform.TryGetValue(target,out var transform))
            {
                Debug.Log("There are no transform");
                return;
            }
            try
            {
                ITargetUI targetui = transform.GetComponent<ITargetUI>();
                if(targetui == null) return;
                targetui.OnCollision();
                _targetToTransform.Remove(target);
            }
            catch (Exception ex)
            {
                Debug.LogError(ex);
                return;
            }
        }
    }
}