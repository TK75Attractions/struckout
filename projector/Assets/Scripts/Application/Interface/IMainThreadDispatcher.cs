using System;

namespace Struckout.Application.Interface
{
    public interface IMainThreadDispatcher
    {
        void Enqueue(Action action);
    }
}