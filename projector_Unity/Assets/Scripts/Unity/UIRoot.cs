using UnityEngine;
namespace Struckout.Unity
{
    public class UIRoot
    {
        public RectTransform TargetRoot { get; private set;}
        public UIRoot(
            RectTransform targetRoot
        )
        {
            TargetRoot = targetRoot;
        }
    }
}