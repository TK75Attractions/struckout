using UnityEngine;
namespace Struckout.Unity
{
    /// <summary>
    /// 的とマーカーを置く親。
    ///
    /// uGUI をやめてスプライトに移したので、型は RectTransform ではなく Transform。
    /// 得点表示などの HUD は引き続き Canvas 側に置くため、盤面の親はこれとは別になる。
    /// </summary>
    public class UIRoot
    {
        public Transform TargetRoot { get; private set;}
        public UIRoot(
            Transform targetRoot
        )
        {
            TargetRoot = targetRoot;
        }
    }
}
