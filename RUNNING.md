### How to build and run an image on MacOS using docker, buildpack and colima

#### install deps
    brew install buildpacks/tap/pack
    brew install colima
    brew install docker

#### build 
    pack config default-builder paketobuildpacks/builder-jammy-full
    pack build tantivy-exploration -b docker.io/paketocommunity/rust
    docker run --rm -p 8080:8080 tantivy-exploration

### To save it:
  docker save tantivy-exploration > tantivy-exploration.tar
