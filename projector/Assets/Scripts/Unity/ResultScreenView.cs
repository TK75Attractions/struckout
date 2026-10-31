using System.Collections.Generic;
using System.Globalization;
using System.Threading;
using Cysharp.Threading.Tasks;
using Tk75Attractions.Struckout.GameMaster.V1;
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
        private readonly Text _context;
        private readonly Text _scoreLabel;
        private readonly Text _rankMessage;
        private readonly GameObject _header;
        private readonly RectTransform _content;
        private readonly Font _textFont;
        private readonly Font _numberFont;
        private readonly List<GameObject> _rows = new();
        private static readonly Color Cyan = new(0.25f, 0.85f, 1f);
        private static readonly Color Gold = new(1f, 0.84f, 0.3f);
        private static readonly Color Silver = new(0.55f, 0.63f, 0.7f);
        private static readonly Color Bronze = new(0.85f, 0.55f, 0.32f);
        private static readonly Color PersonalBackground = new(0.04f, 0.18f, 0.25f, 1f);
        private static readonly Color RowBackground = new(0.055f, 0.065f, 0.09f, 1f);
        private const float RankX = 24f;
        private const float RankWidth = 150f;
        private const float NameX = 200f;
        private const float NameWidth = 850f;
        private const float CurrentX = 1070f;
        private const float CurrentWidth = 160f;
        private const float ScoreX = 1250f;
        private const float ScoreWidth = 486f;
        private const double CountUpSeconds = 1;
        private int _animationVersion;

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
            _context = Label("Context", rect, 800, 40, 1040, 100, 32);
            _context.alignment = TextAnchor.MiddleRight;
            _context.color = Cyan;
            var accent = new GameObject("Accent", typeof(RectTransform), typeof(Image));
            Place(accent.GetComponent<RectTransform>(), rect, 80, 145, 1760, 3);
            accent.GetComponent<Image>().color = Cyan;
            _message = Label("Status", rect, 80, 450, 1760, 120, 38);
            _message.alignment = TextAnchor.MiddleCenter;
            _score = Label("ResultScore", rect, 80, 330, 1760, 300, 180, numberFont);
            _score.alignment = TextAnchor.MiddleCenter;
            _score.color = Cyan;
            _scoreLabel = Label("ScoreLabel", rect, 80, 265, 1760, 65, 42, numberFont);
            _scoreLabel.text = "SCORE";
            _scoreLabel.alignment = TextAnchor.MiddleCenter;
            _rankMessage = Label("RankMessage", rect, 80, 660, 1760, 100, 52);
            _rankMessage.alignment = TextAnchor.MiddleCenter;
            _rankMessage.color = Gold;

            _header = new GameObject("RankingHeader", typeof(RectTransform));
            var headerRect = _header.GetComponent<RectTransform>();
            Place(headerRect, rect, 80, 175, 1760, 50);
            Label("RankHeader", headerRect, RankX, 0, RankWidth, 50, 28).text = "順位";
            Label("PlayerHeader", headerRect, NameX, 0, NameWidth, 50, 28).text = "プレイヤー";
            var scoreHeader = Label("ScoreHeader", headerRect, ScoreX, 0, ScoreWidth, 50, 28);
            scoreHeader.text = "スコア";
            scoreHeader.alignment = TextAnchor.MiddleRight;

            var list = new GameObject("RankingList", typeof(RectTransform));
            _content = list.GetComponent<RectTransform>();
            Place(_content, rect, 80, 235, 1760, 730);
            _root.SetActive(false);
        }

        public void SetScreen(ProjectorScreen screen)
        {
            _animationVersion++;
            _root.SetActive(screen == ProjectorScreen.Score || screen == ProjectorScreen.Ranking);
            _title.text = screen == ProjectorScreen.Score ? "今回の結果" : "ランキング";
            _score.gameObject.SetActive(false);
            _scoreLabel.gameObject.SetActive(false);
            _rankMessage.text = "";
            _context.text = "";
            _header.SetActive(screen == ProjectorScreen.Ranking);
            _content.gameObject.SetActive(false);
            _message.text = "";
            ClearRows();
        }

        public void ShowMessage(string message)
        {
            _animationVersion++;
            _message.text = message;
            _score.gameObject.SetActive(false);
            _scoreLabel.gameObject.SetActive(false);
            _rankMessage.text = "";
            _content.gameObject.SetActive(false);
        }

        public UniTask ShowScoreAsync(uint score, CancellationToken token)
        {
            _message.text = "";
            _score.text = "0";
            _score.gameObject.SetActive(true);
            _scoreLabel.gameObject.SetActive(true);
            return AnimateScoreAsync(score, ++_animationVersion, token);
        }

        public void ShowContext(GameMode mode, Difficulty difficulty, uint? rank)
        {
            var modeName = mode switch
            {
                GameMode.Single => "1人でプレイ",
                GameMode.Multi => "2人で協力プレイ",
                _ => "モード不明",
            };
            var difficultyName = difficulty switch
            {
                Difficulty.Normal => "NORMAL",
                Difficulty.Hard => "HARD",
                Difficulty.Veryhard => "VERY HARD",
                _ => "難易度不明",
            };
            _context.text = modeName + " / " + difficultyName;
            _rankMessage.text = rank.HasValue ? $"ランキング {rank.Value} 位！" : "";
        }

        private async UniTask AnimateScoreAsync(uint score, int version, CancellationToken token)
        {
            // A version invalidates pending frames on screen changes without involving UIService's HUD loop.
            var started = Time.realtimeSinceStartupAsDouble;
            while (_root != null && version == _animationVersion && !token.IsCancellationRequested)
            {
                var progress = System.Math.Min(1, (Time.realtimeSinceStartupAsDouble - started) / CountUpSeconds);
                _score.text = ((uint)(score * progress)).ToString(CultureInfo.InvariantCulture);
                if (progress >= 1) return;
                await UniTask.Yield(cancellationToken: token);
            }
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

            for (var i = 0; i < rows.Count; i++)
            {
                var row = rows[i];
                var go = new GameObject("RankingRow", typeof(RectTransform), typeof(Image));
                var rect = go.GetComponent<RectTransform>();
                Place(rect, _content, 0, i * pitch, cellWidth, cellHeight);
                go.GetComponent<Image>().color = row.IsPersonal
                    ? PersonalBackground : RowBackground;
                var rank = Label("Rank", rect, RankX, 0, RankWidth, cellHeight, 32);
                rank.text = row.Rank.HasValue ? row.Rank.Value + "位" : "圏外";
                var rankColor = row.Rank switch { 1 => Gold, 2 => Silver, 3 => Bronze, _ => Color.white };
                rank.color = rankColor;
                rank.fontStyle = FontStyle.Bold;
                var name = Label("PlayerName", rect, NameX, 0, NameWidth, cellHeight, 30);
                name.text = row.IsPersonal
                    ? string.IsNullOrEmpty(row.PlayerName) ? "あなた" : row.PlayerName + "（あなた）"
                    : row.PlayerName;
                name.color = row.IsPersonal ? Cyan : Color.white;
                FitText(name);
                var current = Label("CurrentGame", rect, CurrentX, 0, CurrentWidth, cellHeight, 24);
                current.text = row.IsCurrentGame ? "今回" : "";
                current.color = Gold;
                FitText(current);
                var score = Label("Score", rect, ScoreX, 0, ScoreWidth, cellHeight, 36, _numberFont);
                score.text = row.Score.ToString(CultureInfo.InvariantCulture);
                score.color = rankColor;
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
