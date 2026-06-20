using UnityEngine;
using Struckout.Domain;

namespace Struckout.Unity
{
    public class CircleTargetUI : MonoBehaviour, ITargetUI
    {
        Target _target;
        public void Initialize(Target target)
        {
            _target = target;
            RectTransform rect = GetComponent<RectTransform>();
            rect.anchoredPosition = new Vector2(target.Coordinate.X,target.Coordinate.Y);
        }

        public void OnCollision()
        {
            Debug.Log("Collision");
            //TODO: Write this book
        }
    }
}