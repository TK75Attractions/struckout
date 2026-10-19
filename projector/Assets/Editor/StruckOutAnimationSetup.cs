using System;
using Struckout.Unity;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace Editor
{
    /// <summary>
    /// Generated artwork のインポート設定と、既存シーン / Prefab の参照を一度に更新する。
    /// 新しい素材に差し替えたときもメニューから再実行できる。
    /// </summary>
    public static class StruckOutAnimationSetup
    {
        private const string ArtRoot = "Assets/Art/StruckOut/";

        [MenuItem("Tools/STRUCK OUT/Apply Projector Animation Art")]
        public static void Apply()
        {
            var background = ImportSprite("arena_background.png");
            var idle = ImportSprite("idle_screen.png");
            var finished = ImportSprite("finished_screen.png");
            var target = ImportSprite("circle_target.png");
            var scored = ImportSprite("hit_scored.png");
            var missed = ImportSprite("hit_missed.png");
            var ignored = ImportSprite("hit_ignored.png");

            const string prefabPath = "Assets/Prefab/CircleTarget.prefab";
            var prefabRoot = PrefabUtility.LoadPrefabContents(prefabPath);
            try
            {
                var renderer = prefabRoot.GetComponent<SpriteRenderer>()
                               ?? throw new InvalidOperationException("CircleTarget prefab needs a SpriteRenderer.");
                renderer.sprite = target;
                renderer.color = Color.white;
                PrefabUtility.SaveAsPrefabAsset(prefabRoot, prefabPath);
            }
            finally
            {
                PrefabUtility.UnloadPrefabContents(prefabRoot);
            }

            const string scenePath = "Assets/Scenes/SampleScene.unity";
            var scene = EditorSceneManager.OpenScene(scenePath, OpenSceneMode.Single);
            var service = UnityEngine.Object.FindAnyObjectByType<UIService>()
                          ?? throw new InvalidOperationException("SampleScene needs UIService.");
            var serialized = new SerializedObject(service);
            SetSprite(serialized, "_backgroundSprite", background);
            SetSprite(serialized, "_idleSprite", idle);
            SetSprite(serialized, "_finishedSprite", finished);
            SetSprite(serialized, "_scoredImpactSprite", scored);
            SetSprite(serialized, "_missedImpactSprite", missed);
            SetSprite(serialized, "_ignoredImpactSprite", ignored);
            serialized.ApplyModifiedPropertiesWithoutUndo();

            var camera = UnityEngine.Object.FindAnyObjectByType<Camera>();
            if (camera != null)
            {
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = Color.black;
            }

            EditorSceneManager.MarkSceneDirty(scene);
            EditorSceneManager.SaveScene(scene);
            AssetDatabase.SaveAssets();
            Debug.Log("[STRUCK OUT] Projector animation art assigned to scene and prefab.");
        }

        public static void ApplyFromCommandLine() => Apply();

        private static Sprite ImportSprite(string name)
        {
            var path = ArtRoot + name;
            AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
            var importer = AssetImporter.GetAtPath(path) as TextureImporter
                           ?? throw new InvalidOperationException($"Texture import failed: {path}");
            importer.textureType = TextureImporterType.Sprite;
            importer.spriteImportMode = SpriteImportMode.Single;
            importer.spritePixelsPerUnit = 100f;
            importer.alphaIsTransparency = true;
            importer.mipmapEnabled = false;
            importer.textureCompression = TextureImporterCompression.Uncompressed;
            importer.filterMode = FilterMode.Bilinear;
            importer.maxTextureSize = 2048;
            importer.SaveAndReimport();

            return AssetDatabase.LoadAssetAtPath<Sprite>(path)
                   ?? throw new InvalidOperationException($"Sprite import failed: {path}");
        }

        private static void SetSprite(SerializedObject owner, string fieldName, Sprite sprite)
        {
            var field = owner.FindProperty(fieldName)
                        ?? throw new InvalidOperationException($"UIService is missing {fieldName}.");
            field.objectReferenceValue = sprite;
        }
    }
}
