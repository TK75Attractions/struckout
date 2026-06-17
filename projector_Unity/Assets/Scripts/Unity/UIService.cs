using UnityEngine;
using Struckout.Domain;
using Struckout.Application;
using System.Collections.Generic;
using System;

namespace Struckout.Unity
{
    public class UIService : MonoBehaviour, IUIService
    {
        private Dictionary<Target, Transform> _targetToTransform = new();
        [SerializeField]
        private Transform _circleUI;
        
        public void InstantiateTargets(IReadOnlyList<Target> targets)
        {
            foreach (var target in targets)
            {
                InstantiateTarget(target);
            }
        }

        public void InstantiateTarget(Target target)
        {
            Transform trans;
            if (_targetToTransform.TryGetValue(target, out var _)) return;

            switch (target.Type)
            {
                case TargetType.Circle:
                    if (_circleUI == null)
                    {
                        UnityEngine.Debug.LogError("CircleUI is null");
                        return;
                    }

                    if(!TryInstantiateTargetUI<CircleTargetUI>(_circleUI, out var transform))
                    {
                        UnityEngine.Debug.LogError("CircleTargetUI is null");
                        return;
                    }
                    trans = transform;
                    break;
                default:
                    UnityEngine.Debug.LogError($"Missing TargetType { target.Type }");
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
                UnityEngine.Debug.LogError("There are no ITargetUI");
                return false;
            }
            
            transform = Instantiate(prefab);
            return true;
        }

        public void OnCollisionTarget(Target target)
        {
            if (!_targetToTransform.TryGetValue(target,out var transform))
            {
                UnityEngine.Debug.Log("There are no transform");
                return;
            }
            try
            {
                ITargetUI targetui = transform.GetComponent<ITargetUI>();
                if(targetui == null) return;
                targetui.OnCollision();
            }
            catch (Exception ex)
            {
                UnityEngine.Debug.LogError(ex);
                return;
            }
        }
    }
}