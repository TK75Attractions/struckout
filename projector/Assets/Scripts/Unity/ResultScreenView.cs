using System.Collections.Generic;
using System.Globalization;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using UnityEngine;
using UnityEngine.UI;

namespace Struckout.Unity
{
    internal sealed class ResultScreenView
    {
        private readonly GameObject _root;
        private readonly Text _title;
        private readonly Text _message;
        private readonly Text _score;
        private readonly Text _description;
        private readonly RectTransform _content;
        private readonly Font _textFont;
        private readonly Font _numberFont;
        private readonly List<GameObject> _rows = new();
        private static readonly Color Cyan = new(0.25f, 0.85f, 1f);

        public ResultScreenView(Canvas canvas, Font textFont, Font numberFont)
        {
            _textFont = textFont;
            _numberFont = numberFont;
            _root = new GameObject("ResultScreens", typeof(RectTransform), typeof(Image));
            var rect = _root.GetComponent<RectTransform>();
            rect.SetParent(canvas.transform, false);
            Stretch(rect);
            _root.GetComponent<Image>().color = new Color(0.015f, 0.02f, 0.04f, 0.96f);
            _title = Label("Title", rect, 80, 40, 1760, 100, 56);
            _title.fontStyle = FontStyle.Bold;
            var accent = new GameObject("Accent", typeof(RectTransform), typeof(Image));
            Place(accent.GetComponent<RectTransform>(), rect, 80, 145, 1760, 3);
            accent.GetComponent<Image>().color = Cyan;
            _description = Label("Description", rect, 80, 160, 1760, 55, 26);
            _description.color = new Color(0.75f, 0.8f, 0.85f);
            _message = Label("Status", rect, 80, 450, 1760, 120, 38);
            _message.alignment = TextAnchor.MiddleCenter;
            _score = Label("ResultScore", rect, 80, 330, 1760, 300, 180, numberFont);
            _score.alignment = TextAnchor.MiddleCenter;
            _score.color = Cyan;
            var footer = Label("Footer", rect, 80, 990, 1760, 55, 24);
            footer.text = "画面の切り替えはタッチパネルから";
            footer.color = new Color(0.65f, 0.7f, 0.75f);

            var list = new GameObject("RankingList", typeof(RectTransform));
            _content = list.GetComponent<RectTransform>();
            Place(_content, rect, 80, 235, 1760, 730);
            _root.SetActive(false);
        }

        public void SetScreen(ProjectorScreen screen)
        {
            _root.SetActive(screen == ProjectorScreen.Score || screen == ProjectorScreen.Ranking);
            _title.text = screen == ProjectorScreen.Score ? "今回の結果" : "ランキング";
            _description.text = screen == ProjectorScreen.Score ? "今回の得点" : $"上位{RankingResults.TopItemCount}件とあなたの履歴を統合し、得点上位{RankingResults.DisplayItemCount}件を表示しています";
            _score.gameObject.SetActive(false);
            _content.gameObject.SetActive(false);
            _message.text = "";
            ClearRows();
        }

        public void ShowMessage(string message)
        {
            _message.text = message;
            _score.gameObject.SetActive(false);
            _content.gameObject.SetActive(false);
        }

        public void ShowScore(uint score)
        {
            _message.text = "";
            _score.text = score.ToString(CultureInfo.InvariantCulture);
            _score.gameObject.SetActive(true);
        }

        public void ShowRanking(IReadOnlyList<RankingRow> rows)
        {
            ClearRows();
            _message.text = rows.Count == 0 ? "ランキングはまだありません。" : "";
            _content.gameObject.SetActive(rows.Count > 0);
            if (rows.Count == 0) return;

            // Reserve twelve fixed rows so text and spacing stay consistent between games.
            var cellWidth = _content.rect.width;
            var pitch = _content.rect.height / RankingResults.DisplayItemCount;
            var cellHeight = pitch - 6f;
            const float padding = 24f;
            const float nameWidth = 950f;
            const float currentWidth = 220f;
            const float scoreWidth = 500f;

            for (var i = 0; i < rows.Count; i++)
            {
                var row = rows[i];
                var go = new GameObject("RankingRow", typeof(RectTransform), typeof(Image));
                var rect = go.GetComponent<RectTransform>();
                Place(rect, _content, 0, i * pitch, cellWidth, cellHeight);
                go.GetComponent<Image>().color = row.IsPersonal
                    ? new Color(0.04f, 0.18f, 0.25f, 1f) : new Color(0.055f, 0.065f, 0.09f, 1f);
                var name = Label("PlayerName", rect, padding, 0, nameWidth, cellHeight, 30);
                name.text = row.IsPersonal
                    ? string.IsNullOrEmpty(row.PlayerName) ? "あなた" : row.PlayerName + "（あなた）"
                    : row.PlayerName;
                name.color = row.IsPersonal ? Cyan : Color.white;
                FitText(name);
                var current = Label("CurrentGame", rect, padding + nameWidth + 8, 0, currentWidth, cellHeight, 24);
                current.text = row.IsCurrentGame ? "今回" : "";
                current.color = new Color(1f, 0.95f, 0.2f);
                FitText(current);
                var score = Label("Score", rect, cellWidth - padding - scoreWidth, 0, scoreWidth, cellHeight, 36, _numberFont);
                score.text = row.Score.ToString(CultureInfo.InvariantCulture);
                score.alignment = TextAnchor.MiddleRight;
                FitText(score);
                _rows.Add(go);
            }
        }

        private static void FitText(Text text)
        {
            text.resizeTextForBestFit = true;
            text.resizeTextMinSize = 1;
            text.resizeTextMaxSize = text.fontSize;
        }

        private void ClearRows()
        {
            foreach (var row in _rows)
            {
                row.SetActive(false);
                Object.Destroy(row);
            }
            _rows.Clear();
        }

        private Text Label(string name, RectTransform parent, float x, float y, float width, float height, int size, Font font = null)
        {
            var go = new GameObject(name, typeof(RectTransform), typeof(Text));
            Place(go.GetComponent<RectTransform>(), parent, x, y, width, height);
            var text = go.GetComponent<Text>();
            text.font = font != null ? font : _textFont;
            text.fontSize = size;
            text.color = Color.white;
            text.alignment = TextAnchor.MiddleLeft;
            text.supportRichText = false;
            text.raycastTarget = false;
            text.horizontalOverflow = HorizontalWrapMode.Wrap;
            text.verticalOverflow = VerticalWrapMode.Truncate;
            return text;
        }

        private static void Place(RectTransform rect, Transform parent, float x, float y, float width, float height)
        {
            rect.SetParent(parent, false);
            rect.anchorMin = rect.anchorMax = rect.pivot = new Vector2(0, 1);
            rect.anchoredPosition = new Vector2(x, -y);
            rect.sizeDelta = new Vector2(width, height);
        }

        private static void Stretch(RectTransform rect)
        {
            rect.anchorMin = Vector2.zero;
            rect.anchorMax = Vector2.one;
            rect.offsetMin = rect.offsetMax = Vector2.zero;
        }
    }
}
