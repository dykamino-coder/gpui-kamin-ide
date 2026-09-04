const INDEX_MEDIA_TYPES = new Set([
  "application/vnd.oci.image.index.v1+json",
  "application/vnd.docker.distribution.manifest.list.v2+json",
]);
const MANIFEST_ACCEPT = [
  "application/vnd.oci.image.index.v1+json",
  "application/vnd.docker.distribution.manifest.list.v2+json",
  "application/vnd.oci.image.manifest.v1+json",
  "application/vnd.docker.distribution.manifest.v2+json",
].join(", ");

function assertDigest(value, field) {
  if (typeof value !== "string" || !/^sha256:[a-f0-9]{64}$/.test(value)) {
    throw new Error(`Invalid ${field}`);
  }
  return value;
}

export function selectLinuxAmd64Manifest(index) {
  const manifests = Array.isArray(index.manifests) ? index.manifests : [];
  const image = manifests.find(
    (item) =>
      item.platform?.os === "linux" && item.platform?.architecture === "amd64",
  );
  if (!image) throw new Error("Image index has no linux/amd64 manifest");
  const attestations = manifests.filter(
    (item) =>
      item.annotations?.["vnd.docker.reference.type"] ===
        "attestation-manifest" &&
      item.annotations?.["vnd.docker.reference.digest"] === image.digest,
  );
  return { image, attestations };
}

export function assertImageLabels(labels, expected) {
  const actual = labels ?? {};
  const required = {
    "org.opencontainers.image.source": expected.source,
    "org.opencontainers.image.revision": expected.revision,
    "org.opencontainers.image.version": expected.version,
  };
  for (const [name, value] of Object.entries(required)) {
    if (actual[name] !== value)
      throw new Error(`Image label ${name} does not match release`);
  }
}

async function fetchJson(url, options, fetchImpl) {
  const response = await fetchImpl(url, options);
  if (!response.ok)
    throw new Error(`Registry request failed (${response.status}): ${url}`);
  return { response, value: await response.json() };
}

export async function inspectDockerHubImage({ image, tag, fetchImpl = fetch }) {
  if (
    !/^[a-z0-9][a-z0-9._/-]+$/.test(image) ||
    !/^[A-Za-z0-9_.-]+$/.test(tag)
  ) {
    throw new Error("Invalid Docker Hub image or tag");
  }
  const scope = encodeURIComponent(`repository:${image}:pull`);
  const tokenResult = await fetchJson(
    `https://auth.docker.io/token?service=registry.docker.io&scope=${scope}`,
    {},
    fetchImpl,
  );
  const token = tokenResult.value.token;
  if (typeof token !== "string" || token.length < 10)
    throw new Error("Docker Hub token is missing");
  const headers = { Authorization: `Bearer ${token}`, Accept: MANIFEST_ACCEPT };
  const base = `https://registry-1.docker.io/v2/${image}`;
  const rootResult = await fetchJson(
    `${base}/manifests/${tag}`,
    { headers },
    fetchImpl,
  );
  const rootDigest = assertDigest(
    rootResult.response.headers.get("docker-content-digest"),
    "root digest",
  );
  const mediaType =
    rootResult.value.mediaType ??
    rootResult.response.headers.get("content-type");

  let manifest = rootResult.value;
  let attestationCount = 0;
  if (INDEX_MEDIA_TYPES.has(mediaType)) {
    const selected = selectLinuxAmd64Manifest(rootResult.value);
    attestationCount = selected.attestations.length;
    const imageResult = await fetchJson(
      `${base}/manifests/${assertDigest(selected.image.digest, "image digest")}`,
      { headers },
      fetchImpl,
    );
    manifest = imageResult.value;
  }
  const configDigest = assertDigest(manifest.config?.digest, "config digest");
  const configResult = await fetchJson(
    `${base}/blobs/${configDigest}`,
    { headers: { Authorization: `Bearer ${token}` } },
    fetchImpl,
  );
  return {
    digest: rootDigest,
    labels: configResult.value.config?.Labels ?? {},
    attestationCount,
    indexed: INDEX_MEDIA_TYPES.has(mediaType),
  };
}
