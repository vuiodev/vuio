export GITHUB_ORG="vuiodev"
export IMAGE_NAME="vuio"
export VERSION_TAG="v0.0.15"

docker login ghcr.io

docker buildx create --name mybuilder --use
docker buildx inspect --bootstrap

docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag ghcr.io/${GITHUB_ORG}/${IMAGE_NAME}:${VERSION_TAG} \
  --tag ghcr.io/${GITHUB_ORG}/${IMAGE_NAME}:latest \
  --output type=image,push=true .

#docker buildx rm mybuilder