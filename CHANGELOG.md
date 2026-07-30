# Changelog

## 0.1.0 (2026-07-30)


### ⚠ BREAKING CHANGES

* return Result instead of panicking on caller data

### Features

* add Gray ([6f40a1a](https://github.com/twangodev/mtb-align/commit/6f40a1a16fbdb50d630ce32bb44e27970143a1cd))
* add median threshold and exclusion bitmaps ([0b2148f](https://github.com/twangodev/mtb-align/commit/0b2148fd8eaaf1e32f6aa38523298a387e584d9b))
* add packed Bitmap ([a10cb42](https://github.com/twangodev/mtb-align/commit/a10cb42538fc96b6f815243387a20597b83bc0d3))
* add pyramid shrink in both conventions ([0c09b33](https://github.com/twangodev/mtb-align/commit/0c09b330091fe0a771b9942fede5a2dc4acbde8c))
* add shifting, common crop, and stack alignment ([a23f9f0](https://github.com/twangodev/mtb-align/commit/a23f9f02a58e5914e50a4c1ff753f72e7f6ff946))
* add the coarse-to-fine alignment search ([df25c71](https://github.com/twangodev/mtb-align/commit/df25c71cbff8d4787a818a189eb006948faae09f))
* add the fused disagreement count ([ed2fd14](https://github.com/twangodev/mtb-align/commit/ed2fd1419716fc079e366a8d799e3791624e2b3e))
* return Result instead of panicking on caller data ([db1916a](https://github.com/twangodev/mtb-align/commit/db1916ae63019ef319a7afd35a9a031460ad8cf3))


### Performance Improvements

* add an optional rayon feature for the row passes ([24f76b9](https://github.com/twangodev/mtb-align/commit/24f76b985ef8b2eb80ba0c5159c389c3f5a589b0))
* pack bitmaps a word at a time instead of a bit at a time ([84eddb0](https://github.com/twangodev/mtb-align/commit/84eddb027163ff4d5c8d13cea355b2bf810bf7e6))
