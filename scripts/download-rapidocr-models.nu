#!/usr/bin/env -S nix develop --command nu
# Pre-download rapidocr OCR models to /tmp/.rapidocr-models.
# Idempotent: skips if models already exist.

def main [] {
    let model_dir: string = "/tmp/.rapidocr-models"
    if ($model_dir | path exists) and ((ls $model_dir | length) > 0) {
        print $"SKIP: ($model_dir) already has models"
        return
    }
    mkdir $model_dir
    ^rapidocr download_models
    print $"OK: models downloaded to ($model_dir)"
}
