using System;
using System.Globalization;
using Struckout.Application.Interface;
using Struckout.Domain;
using Struckout.Infrastructure;
using UnityEngine;

namespace Struckout.Unity
{
    /// <summary>
    /// 物理座標 → 描画座標の係数 (<see cref="CollisionCoordinateTransform"/>) を、実行中に合わせ込むための画面。
    /// 書き出したプレイヤーでも使えるよう IMGUI で描き、F2 で開閉する。
    ///
    /// - 係数を直接入れる。矢印キーで原点を、Ctrl + 矢印キーで 1 m あたりの値を動かす (Shift で大きく)
    /// - 「合わせ用の 2 点」を出し、板の上で実際に映った位置 (左の壁から、地面から。m) を入れると係数を計算する
    /// - 板の 4 隅と中央 (実測した位置) に印を出し続けて、合っているか確かめる
    /// - 設定ファイルに保存する (<see cref="CollisionTransformStore"/>)。次の起動からその値を使う
    ///
    /// 係数は SensorProvider と同じインスタンスを書き換えるので、変えたらすぐ ball_tracker からの着弾に効く。
    /// </summary>
    public class CollisionCalibrationPanel : MonoBehaviour
    {
        /// <summary>
        /// 確かめに使う板の点 (左の壁からの距離, 地面からの高さ。m)。2026-10-16 に測った、奥の板の上の段の 4 隅
        /// (左右の黒い柱の内側の縁と、上の梁の下の縁・上下の板の継ぎ目が交わる点) と、その真ん中。
        /// </summary>
        private static readonly (string Name, double X, double Y)[] CheckPoints =
        {
            ("左上", 0.07, 2.47), ("右上", 1.13, 2.47), ("左下", 0.07, 1.79), ("右下", 1.13, 1.79), ("中央", 0.60, 2.13),
        };

        /// <summary>合わせ用の 2 点 (盤面の描画座標)。盤面 1920x1080 の 1/4 と 3/4。</summary>
        private static readonly (double X, double Y) FitRender1 = (480, 270);
        private static readonly (double X, double Y) FitRender2 = (1440, 810);

        private const float MarkerInterval = 0.5f;

        private CollisionCoordinateTransform _transform;
        private CollisionCoordinateTransform _inspectorValues;
        private IUIService _ui;
        private string _path;

        private bool _visible;
        private bool _showCheckPoints;
        private bool _showFitPoints;
        private float _nextMarkerTime;
        private string _status = "";
        private Vector2 _scroll;

        // 入力中の文字列 (数として読めたときだけ係数に入れる)
        private string _ppmX, _ppmY, _originX, _originY;
        private string _m1X = "", _m1Y = "", _m2X = "", _m2Y = "";

        /// <param name="inspectorValues">「Inspector の値に戻す」で戻す値 (設定ファイルを読む前の値)</param>
        public void Construct(CollisionCoordinateTransform transform, CollisionCoordinateTransform inspectorValues, IUIService ui, string path)
        {
            _transform = transform ?? throw new ArgumentNullException(nameof(transform));
            _inspectorValues = inspectorValues;
            _ui = ui;
            _path = path;
            RefreshFields();
        }

        private void Update()
        {
            if (_transform == null || _ui == null || (!_showCheckPoints && !_showFitPoints)) return;
            if (Time.unscaledTime < _nextMarkerTime) return;
            _nextMarkerTime = Time.unscaledTime + MarkerInterval;

            // マーカーは 1 秒で消えるので、出し続ける
            if (_showCheckPoints)
            {
                foreach (var (_, x, y) in CheckPoints)
                {
                    var (rx, ry) = _transform.ToRenderSpace(x, y);
                    _ui.ShowCollisionMarker((float)rx, (float)ry, CollisionResult.Scored);
                }
            }

            if (_showFitPoints)
            {
                _ui.ShowCollisionMarker((float)FitRender1.X, (float)FitRender1.Y, CollisionResult.Missed);
                _ui.ShowCollisionMarker((float)FitRender2.X, (float)FitRender2.Y, CollisionResult.Missed);
            }
        }

        private void OnGUI()
        {
            var e = Event.current;
            if (e.type == EventType.KeyDown && e.keyCode == KeyCode.F2)
            {
                _visible = !_visible;
                e.Use();
            }

            if (!_visible || _transform == null) return;

            HandleNudgeKeys(e);

            // 画面の高さ 1080 で文字が読める大きさにする
            var scale = Mathf.Max(1f, Screen.height / 720f);
            GUI.matrix = Matrix4x4.Scale(new Vector3(scale, scale, 1f));
            var area = new Rect(10, 10, 470, Screen.height / scale - 20);
            GUILayout.BeginArea(area, GUI.skin.box);
            _scroll = GUILayout.BeginScrollView(_scroll);

            GUILayout.Label("着弾の座標の係数 (F2 で閉じる)");
            GUILayout.Label("矢印: 原点を 1 px / Ctrl+矢印: 1 m あたりを 0.5% (Shift で 10 倍)");
            GUILayout.Label("入力欄に打ったあとは Esc で抜けると矢印キーが効く");

            _ppmX = NumberField("1 m あたり 横 (px)", _ppmX, v => _transform.pixelsPerMetreX = v, positive: true);
            _ppmY = NumberField("1 m あたり 縦 (px)", _ppmY, v => _transform.pixelsPerMetreY = v, positive: true);
            _originX = NumberField("原点 横 (px)", _originX, v => _transform.originX = v);
            _originY = NumberField("原点 縦 (px)", _originY, v => _transform.originY = v);
            GUILayout.BeginHorizontal();
            _transform.flipX = GUILayout.Toggle(_transform.flipX, "横を反転");
            _transform.flipY = GUILayout.Toggle(_transform.flipY, "縦を反転");
            _transform.swapAxes = GUILayout.Toggle(_transform.swapAxes, "縦横を入れ替え");
            GUILayout.EndHorizontal();
            _transform.logConversions = GUILayout.Toggle(_transform.logConversions, "変換をログに出す");

            GUILayout.Space(8);
            _showCheckPoints = GUILayout.Toggle(_showCheckPoints, "板の 4 隅と中央に印を出す (確かめ用)");
            _showFitPoints = GUILayout.Toggle(_showFitPoints, "合わせ用の 2 点に印を出す");

            GUILayout.Space(8);
            GUILayout.Label("合わせ用の 2 点が板の上で映った位置 (m)");
            GUILayout.Label("横 = 左の壁からの距離、縦 = 地面からの高さ。1 = 左下の点、2 = 右上の点");
            GUILayout.BeginHorizontal();
            GUILayout.Label("1:", GUILayout.Width(20));
            _m1X = GUILayout.TextField(_m1X, GUILayout.Width(80));
            _m1Y = GUILayout.TextField(_m1Y, GUILayout.Width(80));
            GUILayout.Label("2:", GUILayout.Width(20));
            _m2X = GUILayout.TextField(_m2X, GUILayout.Width(80));
            _m2Y = GUILayout.TextField(_m2Y, GUILayout.Width(80));
            GUILayout.EndHorizontal();
            if (GUILayout.Button("2 点から係数を計算して入れる")) FitFromMeasurements();

            GUILayout.Space(8);
            GUILayout.BeginHorizontal();
            if (GUILayout.Button("保存")) Save();
            if (GUILayout.Button("ファイルから読み直す")) Reload();
            if (GUILayout.Button("Inspector の値に戻す")) ResetToInspector();
            GUILayout.EndHorizontal();
            GUILayout.Label($"ファイル: {_path}");
            if (!string.IsNullOrEmpty(_status)) GUILayout.Label(_status);

            GUILayout.EndScrollView();
            GUILayout.EndArea();
        }

        private void HandleNudgeKeys(Event e)
        {
            if (e.type != EventType.KeyDown) return;

            // Esc で入力欄から抜ける。入力欄に文字を打っている間は、矢印キーを入力欄に任せる
            if (e.keyCode == KeyCode.Escape)
            {
                GUIUtility.keyboardControl = 0;
                e.Use();
                return;
            }

            if (GUIUtility.keyboardControl != 0) return;
            var dx = e.keyCode == KeyCode.RightArrow ? 1 : e.keyCode == KeyCode.LeftArrow ? -1 : 0;
            var dy = e.keyCode == KeyCode.UpArrow ? 1 : e.keyCode == KeyCode.DownArrow ? -1 : 0;
            if (dx == 0 && dy == 0) return;

            var big = e.shift ? 10f : 1f;
            if (e.control)
            {
                var factor = 1f + 0.005f * big;
                if (dx != 0) _transform.pixelsPerMetreX *= dx > 0 ? factor : 1f / factor;
                if (dy != 0) _transform.pixelsPerMetreY *= dy > 0 ? factor : 1f / factor;
            }
            else
            {
                _transform.originX += dx * big;
                _transform.originY += dy * big;
            }

            RefreshFields();
            e.Use();
        }

        private static string NumberField(string label, string text, Action<float> apply, bool positive = false)
        {
            GUILayout.BeginHorizontal();
            GUILayout.Label(label, GUILayout.Width(180));
            var edited = GUILayout.TextField(text ?? "", GUILayout.Width(150));
            GUILayout.EndHorizontal();
            if (edited != text && TryParse(edited, out var value) && (!positive || value > 0f)) apply(value);
            return edited;
        }

        private static bool TryParse(string text, out float value) =>
            float.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out value);

        private void RefreshFields()
        {
            _ppmX = Format(_transform.pixelsPerMetreX);
            _ppmY = Format(_transform.pixelsPerMetreY);
            _originX = Format(_transform.originX);
            _originY = Format(_transform.originY);
        }

        private static string Format(float value) => value.ToString("0.###", CultureInfo.InvariantCulture);

        private void FitFromMeasurements()
        {
            if (!TryParse(_m1X, out var x1) || !TryParse(_m1Y, out var y1) || !TryParse(_m2X, out var x2) || !TryParse(_m2Y, out var y2))
            {
                _status = "2 点の位置を数で入れてください (m)";
                return;
            }

            try
            {
                _transform.FitTwoPoints(FitRender1, (x1, y1), FitRender2, (x2, y2));
                RefreshFields();
                _status = $"2 点から計算しました: {_transform}";
                Debug.Log($"[Collision] fitted from two points: {_transform}");
            }
            catch (ArgumentException ex)
            {
                _status = $"計算できません: {ex.Message}";
            }
        }

        private void Save()
        {
            try
            {
                CollisionTransformStore.Save(_transform, _path);
                _status = $"保存しました ({DateTime.Now:HH:mm:ss})";
                Debug.Log($"[Collision] saved {_transform} to {_path}");
            }
            catch (Exception ex) when (ex is System.IO.IOException || ex is UnauthorizedAccessException)
            {
                _status = $"保存できません: {ex.Message}";
            }
        }

        private void Reload()
        {
            if (CollisionTransformStore.TryLoadInto(_transform, _path, out var error))
            {
                _status = "ファイルから読み直しました";
            }
            else
            {
                _status = error != null ? $"読めません: {error}" : "ファイルがありません";
            }

            RefreshFields();
        }

        private void ResetToInspector()
        {
            if (_inspectorValues == null) return;
            _transform.CopyFrom(_inspectorValues);
            RefreshFields();
            _status = "Inspector の値に戻しました (保存はしていません)";
        }
    }
}
