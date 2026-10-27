namespace Struckout.Domain.TargetDefinition
{
    /// <summary>
    /// The supplied 1254px sprites share a centered 930px dial. Keep the canvas padding
    /// when drawing both layers so the hand pivot and circular hit area coincide.
    /// </summary>
    public static class PocketWatchLayout
    {
        public const float CanvasToDialRatio = 1254f / 930f;
    }
}
