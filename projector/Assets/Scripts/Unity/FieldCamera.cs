using Struckout.Domain;
using UnityEngine;
using VContainer;

namespace Struckout.Unity
{
    /// <summary>
    /// 盤面がちょうど収まるようにカメラの orthographicSize を合わせる。
    ///
    /// 盤面の広さ (px) と 1 unit あたりのピクセル数を決めたら、カメラの
    /// 表示範囲は一意に決まる。手で両方を設定すると必ずいつかずれるので、
    /// <see cref="WorldCoordinateTransform"/> から導いてここで当てている。
    /// Inspector の orthographicSize は実行時に上書きされる。
    ///
    /// 投影面の比率が盤面と違う場合は、的が画面外に出ないほうに寄せる
    /// (横長なら左右に、縦長なら上下に余白が出る)。以前の CanvasScaler は
    /// 幅基準だったため、縦長の投影面では上下が切れていた。
    /// </summary>
    [RequireComponent(typeof(Camera))]
    public class FieldCamera : MonoBehaviour
    {
        private Camera _camera;
        private WorldCoordinateTransform _world;

        /// <summary>直前に適用したときの比率。変わったときだけ計算し直す。</summary>
        private float _appliedAspect = -1f;

        [Inject]
        public void Construct(WorldCoordinateTransform world)
        {
            _world = world;
            Apply();
        }

        private void Awake()
        {
            _camera = GetComponent<Camera>();
        }

        // 解像度やディスプレイが変わることがあるので、毎フレーム比率だけ見る。
        private void LateUpdate() => Apply();

        private void Apply()
        {
            if (_world == null) return;
            if (_camera == null) _camera = GetComponent<Camera>();
            if (_camera == null) return;

            float aspect = _camera.aspect;
            if (aspect <= 0f || Mathf.Approximately(aspect, _appliedAspect)) return;

            _camera.orthographic = true;
            _camera.orthographicSize = _world.OrthographicSizeFor(aspect);
            _appliedAspect = aspect;

            Debug.Log(
                $"[Field] {_world} aspect={aspect:F3} orthographicSize={_camera.orthographicSize:F3}");
        }
    }
}
