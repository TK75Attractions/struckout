using UnityEngine;
using Struckout.Domain;
using System.Drawing;

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

            // Treat Target.Size as the radius of the circle.
            rect.localScale = new Vector3(target.Radius, target.Radius, 1);
        }

        public void OnCollision()
        {
            Debug.Log("Collision");
            Destroy(gameObject);
            //TODO: Write this book
        }
    }
}