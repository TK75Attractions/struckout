using System;
using Tk75Attractions.Struckout.GameMaster.V1;

namespace Struckout.Domain
{
    /// <summary>
    /// Extension methods for <see cref="Target"/>>.
    /// </summary>
    public static class TargetExtensions
    {
        /// <returns>Whether the target is owned by the machine.</returns>
        /// <exception cref="ArgumentOutOfRangeException"></exception>
        public static bool IsOwnedBy(this Target target, uint machineId)
        {
            return target.OwnerCase switch
            {
                Target.OwnerOneofCase.None => false,
                Target.OwnerOneofCase.Single => target.Single.MachineId == machineId,
                Target.OwnerOneofCase.All => true,
                _ => throw new ArgumentOutOfRangeException()
            };
        }
    }
}