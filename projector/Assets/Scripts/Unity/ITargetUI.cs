using Struckout.Domain;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Unity
{
    public interface ITargetUI
    {
        /// <summary>登録済みの UUID に対応する新しい描画オブジェクトを初期化する。</summary>
        void Initialize(Target target, WorldCoordinateTransform world);

        /// <summary>この UUID の描画を削除する。</summary>
        void Remove();
    }
}
