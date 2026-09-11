#!/usr/bin/env -S nix develop --command nu

def main [] {
    let model_dir: string = "/tmp/.rapidocr-models"
    if ($model_dir | path exists) and ((ls $model_dir | length) > 0) {
        return
    }
    mkdir $model_dir
    ^rapidocr download_models
}
