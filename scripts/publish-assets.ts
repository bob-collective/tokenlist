import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import {
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
  S3ServiceException,
} from '@aws-sdk/client-s3';
import { glob } from 'glob';
import {
  CHAIN_DIR,
  TOKEN_DIR,
  TOKENLIST_ASSET_PREFIX,
  TOKENLIST_BASE_URL,
} from '../config';

const CACHE_CONTROL = 'public, max-age=300';
const MAX_CONCURRENCY = 8;
const CONTENT_TYPES = {
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
} as const;

type Asset = {
  body: Buffer;
  contentType: (typeof CONTENT_TYPES)[keyof typeof CONTENT_TYPES];
  hash: string;
  key: string;
  path: string;
  url: string;
};

function requireEnvironmentVariable(name: string): string {
  const value = process.env[name];

  if (!value) throw new Error(`Missing required environment variable: ${name}`);

  return value;
}

function normalizePath(filePath: string): string {
  return filePath.split(path.sep).join('/').replace(/^\.\//, '');
}

function getContentType(filePath: string): Asset['contentType'] {
  const extension = path.extname(filePath) as keyof typeof CONTENT_TYPES;
  const contentType = CONTENT_TYPES[extension];

  if (!contentType) throw new Error(`Unsupported asset type: ${filePath}`);

  return contentType;
}

async function loadAssets(): Promise<Asset[]> {
  const filePaths = [
    ...glob.sync(path.join(TOKEN_DIR, '*', 'logo.{svg,webp}')),
    ...glob.sync(path.join(CHAIN_DIR, '*.svg')),
  ].sort();

  return Promise.all(
    filePaths.map(async (filePath) => {
      const normalizedPath = normalizePath(filePath);
      const body = await readFile(filePath);
      const hash = createHash('sha256').update(body).digest('hex');
      const url = new URL(normalizedPath, TOKENLIST_BASE_URL);

      url.searchParams.set('sha256', hash);

      return {
        body,
        contentType: getContentType(filePath),
        hash,
        key: path.posix.join(TOKENLIST_ASSET_PREFIX, normalizedPath),
        path: normalizedPath,
        url: url.toString(),
      };
    }),
  );
}

function isNotFound(error: unknown): boolean {
  return (
    error instanceof S3ServiceException &&
    (error.name === 'NotFound' || error.$metadata.httpStatusCode === 404)
  );
}

async function publishAsset(
  client: S3Client,
  bucket: string,
  asset: Asset,
): Promise<'skipped' | 'uploaded'> {
  try {
    const existing = await client.send(
      new HeadObjectCommand({ Bucket: bucket, Key: asset.key }),
    );

    if (
      existing.Metadata?.sha256 === asset.hash &&
      existing.CacheControl === CACHE_CONTROL &&
      existing.ContentType === asset.contentType
    ) {
      return 'skipped';
    }
  } catch (error) {
    if (!isNotFound(error)) throw error;
  }

  await client.send(
    new PutObjectCommand({
      Body: asset.body,
      Bucket: bucket,
      CacheControl: CACHE_CONTROL,
      ContentType: asset.contentType,
      Key: asset.key,
      Metadata: { sha256: asset.hash },
    }),
  );

  return 'uploaded';
}

async function publishAssets(
  client: S3Client,
  bucket: string,
  assets: Asset[],
): Promise<{ skipped: number; uploaded: number }> {
  let skipped = 0;
  let uploaded = 0;
  const workerCount = Math.min(MAX_CONCURRENCY, assets.length);

  await Promise.all(
    Array.from({ length: workerCount }, async (_, workerIndex) => {
      for (
        let index = workerIndex;
        index < assets.length;
        index += workerCount
      ) {
        const result = await publishAsset(client, bucket, assets[index]);

        if (result === 'uploaded') uploaded += 1;
        else skipped += 1;
      }
    }),
  );

  return { skipped, uploaded };
}

async function verifyAssets(assets: Asset[]): Promise<void> {
  for (const asset of assets) {
    const response = await fetch(asset.url, { cache: 'no-store' });

    if (!response.ok) {
      throw new Error(
        `Asset verification failed (${response.status}): ${asset.path}`,
      );
    }

    const contentType = response.headers.get('content-type');
    if (contentType !== asset.contentType) {
      throw new Error(
        `Asset content type mismatch for ${asset.path}: expected ${asset.contentType}, received ${contentType}`,
      );
    }

    const body = Buffer.from(await response.arrayBuffer());
    const hash = createHash('sha256').update(body).digest('hex');

    if (hash !== asset.hash) {
      throw new Error(`Asset content mismatch: ${asset.path}`);
    }
  }
}

const accountId = requireEnvironmentVariable('R2_ACCOUNT_ID');
const accessKeyId = requireEnvironmentVariable('R2_ACCESS_KEY_ID');
const secretAccessKey = requireEnvironmentVariable('R2_SECRET_ACCESS_KEY');
const bucket = requireEnvironmentVariable('R2_BUCKET_NAME');
const assets = await loadAssets();

if (assets.length === 0) throw new Error('No tokenlist assets found');

const client = new S3Client({
  credentials: { accessKeyId, secretAccessKey },
  endpoint: `https://${accountId}.r2.cloudflarestorage.com`,
  region: 'auto',
});
const result = await publishAssets(client, bucket, assets);

console.log(
  `R2 assets: ${result.uploaded} uploaded, ${result.skipped} unchanged`,
);

await verifyAssets(assets);

console.log(`Verified ${assets.length} assets at ${TOKENLIST_BASE_URL}`);
