using System;
using System.Collections.Generic;
using Struckout.Domain;
using Struckout.Domain.Ranking;
using Struckout.Domain.TargetDefinition;

namespace Struckout.Application.Interface
{
    public interface IUIService
    {
        /// <summary>サーバーからの通知に対応する画面を表示する。</summary>
        void SetScreen(ProjectorScreen screen);
        void ShowCountdown(uint seconds);
        void ShowGameResult(uint score);
        void ShowResultContext(Tk75Attractions.Struckout.GameMaster.V1.GameMode mode,
            Tk75Attractions.Struckout.GameMaster.V1.Difficulty difficulty, uint? rank = null);
        void ShowRanking(IReadOnlyList<RankingRow> rows);
        void ShowScreenMessage(string message);

        /// <summary>game_master から受信した残り時間を表示する。</summary>
        void SetRemainingTime(TimeSpan remaining);

        /// <summary>Starts the shared watch clock before targets can appear.</summary>
        void StartGameClock(TimeSpan duration);

        /// <summary>Watch dial diameter in board pixels, shared by placement and rendering.</summary>
        float PocketWatchDiameter { get; }

        /// <summary>game_master が確定した累計得点を表示する。</summary>
        void SetScore(uint totalScore);

        /// <summary>得点差分を、命中した的またはコンボ表示の位置に一時表示する。</summary>
        void ShowScoreChange(ScoreChange change);

        /// <summary>コンボ数を表示する。2 未満は隠す。</summary>
        void SetCombo(uint combo);

        /// <summary>TargetAdded で通知された UUID を持つ的を描画する。</summary>
        void AddTarget(Target target);

        /// <summary>
        /// Removes target from UI.
        /// </summary>
        /// <param name="id">Id of the target to remove.</param>
        /// <returns>Whether target existed and was removed successfully or not.</returns>
        /// <exception cref="InvalidOperationException"></exception>
        bool RemoveTarget(Guid id);

        /// <summary>
        /// ボールが当たった位置を描画座標で受け取り、その場にマーカーを出す。
        /// 当たり判定に関係なく呼ばれる。外れた位置が見えないと変換係数のずれに気づけないため。
        /// </summary>
        void ShowCollisionMarker(float x, float y, CollisionResult result);
    }
}
