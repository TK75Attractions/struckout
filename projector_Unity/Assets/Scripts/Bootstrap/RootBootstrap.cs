using System;
using UnityEngine;

namespace Struckout.Bootstrap
{
    public class RootBootstrap : MonoBehaviour
    {
        private RuntimeContext runtimeContext = new();

        private void Start()
        {
            
        }
        

        private void OnDestroy()
        {
            runtimeContext.destroyEvent?.Invoke();
        }
    }
}