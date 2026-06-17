using UnityEngine;
using Struckout.Domain;
using Struckout.Application;
using System.Collections.Generic;
using Unity.VisualScripting;
using System;
using System.Linq;

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
            if (_targetToTransform.Keys.Contains(target)) return;

            switch (target.Type)
            {
                case TargetType.Circle:
                    if (_circleUI == null) return;
                    try
                    {
                        if(!TryInstantiateTargetUI<CircleTargetUI>(_circleUI, out var transform))
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
                    UnityEngine.Debug.LogError($"Missing TargetType { target.Type }");
                    return;
            }
            var ui = trans.GetComponent<ITargetUI>() ?? throw new Exception("Doesn't Contain ITargetUI");
            ui.Initialize(target);
            _targetToTransform[target] = trans;
        }

        bool TryInstantiateTargetUI<TTargetUI>(Transform prefab, out Transform transform) where TTargetUI : MonoBehaviour, ITargetUI
        {
            try
            {
                if(prefab.GetComponent<TTargetUI>() == null)
                {
                    throw new Exception("There are no ITargetUI");
                }
            }
            catch
            {
                throw;
            }
            
            transform = Instantiate(prefab);
            return true;
        }

        public void OnCollisionTarget(Target target)
        {
            if (_targetToTransform.TryGetValue(target,out var transform))
            {
                throw new Exception("There are no transform");
            }
            try
            {
                ITargetUI targetui = transform.GetComponent<ITargetUI>();
                targetui.OnCollision();
            }
            catch
            {
                throw;
            }
        }
    }
}