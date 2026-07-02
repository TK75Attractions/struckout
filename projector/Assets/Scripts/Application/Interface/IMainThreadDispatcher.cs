using System;

namespace Struckout.Application
{
    public interface IMainThreadDispatcher
    {
        void Enqueue(Action action);
    }
}