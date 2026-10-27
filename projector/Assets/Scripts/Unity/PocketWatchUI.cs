using Struckout.Domain;
using Struckout.Domain.TargetDefinition;
using UnityEngine;

namespace Struckout.Unity
{
    public sealed class PocketWatchUI : MonoBehaviour, ITargetUI
    {
        private Transform _hand;
        private GameClock _clock;

        public void Configure(Sprite face, Sprite hand, GameClock clock)
        {
            _clock = clock;
            CreateLayer("Face", face, 0);
            _hand = CreateLayer("Hand", hand, 1);
        }

        private Transform CreateLayer(string label, Sprite sprite, int order)
        {
            var layer = new GameObject(label, typeof(SpriteRenderer));
            layer.transform.SetParent(transform, false);
            var renderer = layer.GetComponent<SpriteRenderer>();
            renderer.sprite = sprite;
            renderer.sortingOrder = order;
            // Use the full canvas, rather than the tight mesh, to align the two supplied images.
            layer.transform.localScale = Vector3.one / (sprite.rect.width / sprite.pixelsPerUnit);
            return layer.transform;
        }

        public void Initialize(Target target, WorldCoordinateTransform world)
        {
            transform.localPosition = new Vector3(world.ToWorldX(target.Coordinate.X), world.ToWorldY(target.Coordinate.Y), 0);
            transform.localScale = Vector3.one * world.ToWorldLength(target.Diameter * PocketWatchLayout.CanvasToDialRatio);
            Update();
        }

        private void Update()
        {
            if (_hand != null)
                _hand.localRotation = Quaternion.Euler(0, 0, -(float)_clock.ClockwiseDegrees(Time.realtimeSinceStartupAsDouble));
        }

        public void Remove() => Destroy(gameObject);
    }
}
