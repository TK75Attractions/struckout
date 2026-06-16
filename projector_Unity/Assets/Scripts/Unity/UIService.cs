using UnityEngine;
using Struckout.Domain;
using Struckout.Application;
using System.Collections.Generic;
using Unity.VisualScripting;
using System;

namespace Struckout.Unity
{
    public class UIService : MonoBehaviour, IUIService
    {
        private Dictionary<Target, Transform> _targetToTransform = new();
        [SerializeField]
        private Transform _circleUI;
        
        public void InstantinateTargets(IReadOnlyList<Target> targets)
        {
            foreach (var target in targets)
            {
                InstantinateTarget(target);
            }
        }

        public void InstantinateTarget(Target target)
        {
            Transform trans;

            switch (target.Type)
            {
                case TargetType.Circle:
                    if (_circleUI == null) return;
                    try
                    {
                        if(!TryInstantinateTargetUI<CircleTargetUI>(_circleUI, out var transform))
                        {
                            return;
                        }
                        trans = transform;
                    }
                    catch (Exception ex)
                    {
                        UnityEngine.Debug.Log(ex);
                        return;
                    }
                    break;
                default:
                    throw new Exception($"Missing TargetType { target.Type }");
            }
            var ui = trans.GetComponent<ITargetUI>() ?? throw new Exception("Doesn't Contain ITargetUI");
            ui.Initialize(target);
            _targetToTransform[target] = trans;
        }

        bool TryInstantinateTargetUI<TTargetUI>(Transform prefab, out Transform transform) where TTargetUI : MonoBehaviour, ITargetUI
        {
            transform = Instantiate(prefab);

            return true;
        }

        public void OnCollisionTarget(Target target)
        {
            if (_targetToTransform.TryGetValue(target,out var transform))
            transform.GetComponent<ITargetUI>().OnCollision();
        }
    }
}