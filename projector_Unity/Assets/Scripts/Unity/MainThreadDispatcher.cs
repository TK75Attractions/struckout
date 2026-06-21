using Struckout.Application;
using System;
using System.Collections.Concurrent;
using UnityEngine;
using System.Diagnostics;

namespace Struckout.Unity
{
    public class MainThreadDispatcher : MonoBehaviour, IMainThreadDispatcher
    {
        private readonly ConcurrentQueue<Action> _actions = new();

        public void Enqueue(Action action)
        {
            _actions.Enqueue(action);
        }

        private void Update()
        {
            var stopwatch = Stopwatch.StartNew();
            while (_actions.Count > 0)
            {
                if (!_actions.TryDequeue(out var result)) continue;
                result?.Invoke();
                if (stopwatch.ElapsedMilliseconds >= 2) break;
            }
        }
    }
}