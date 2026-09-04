import assert from "node:assert/strict";
import test from "node:test";
import {
  assertImageLabels,
  inspectDockerHubImage,
  selectLinuxAmd64Manifest,
} from "./docker-registry.mjs";

const digest = (character) => `sha256:${character.repeat(64)}`;

test("selects the runtime manifest and records attestations", () => {
  const index = {
    manifests: [
      { digest: digest("a"), platform: { os: "linux", architecture: "amd64" } },
      {
        digest: digest("b"),
        platform: { os: "unknown", architecture: "unknown" },
        annotations: {
          "vnd.docker.reference.type": "attestation-manifest",
          "vnd.docker.reference.digest": digest("a"),
        },
      },
    ],
  };
  const selected = selectLinuxAmd64Manifest(index);
  assert.equal(selected.image.digest, digest("a"));
  assert.equal(selected.attestations.length, 1);
});

test("reads an indexed Docker Hub image and its config labels", async () => {
  const labels = {
    "org.opencontainers.image.source": "https://github.com/example/repo",
    "org.opencontainers.image.revision": "1".repeat(40),
    "org.opencontainers.image.version": "1.2.3",
  };
  const responses = [
    new Response(JSON.stringify({ token: "a-valid-registry-token" })),
    new Response(
      JSON.stringify({
        mediaType: "application/vnd.oci.image.index.v1+json",
        manifests: [
          {
            digest: digest("a"),
            platform: { os: "linux", architecture: "amd64" },
          },
          {
            digest: digest("b"),
            platform: { os: "unknown", architecture: "unknown" },
            annotations: {
              "vnd.docker.reference.type": "attestation-manifest",
              "vnd.docker.reference.digest": digest("a"),
            },
          },
        ],
      }),
      { headers: { "docker-content-digest": digest("d") } },
    ),
    new Response(
      JSON.stringify({
        mediaType: "application/vnd.oci.image.manifest.v1+json",
        config: { digest: digest("c") },
      }),
    ),
    new Response(JSON.stringify({ config: { Labels: labels } })),
  ];
  const result = await inspectDockerHubImage({
    image: "example/repo",
    tag: "1.2.3",
    fetchImpl: async () => responses.shift(),
  });
  assert.equal(result.digest, digest("d"));
  assert.equal(result.attestationCount, 1);
  assert.equal(result.indexed, true);
  assertImageLabels(result.labels, {
    source: labels["org.opencontainers.image.source"],
    revision: labels["org.opencontainers.image.revision"],
    version: labels["org.opencontainers.image.version"],
  });
});

test("rejects image labels from another release", () => {
  assert.throws(
    () =>
      assertImageLabels(
        { "org.opencontainers.image.revision": "old" },
        { source: "source", revision: "new", version: "1.2.3" },
      ),
    /does not match release/,
  );
});
