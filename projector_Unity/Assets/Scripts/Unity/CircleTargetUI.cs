using UnityEngine;
using Struckout.Domain;

namespace Struckout.Unity
{
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        public void Initialize(Target target)
        {
            transform.position = new Vector2(target.Coordinate.X,target.Coordinate.Y);
        }

        public void OnCollision()
        {
            //TODO: Write this book
        }
    }
}