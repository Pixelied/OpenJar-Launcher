import { useEffect, useState, type ReactNode } from "react";

import { readLocalImageDataUrl } from "../../tauri";

const LOCAL_IMAGE_DATA_URL_CACHE = new Map<string, string>();
const LOCAL_IMAGE_DATA_URL_PENDING = new Map<string, Promise<string | null>>();

function isDirectImageSrc(value: string) {
  return /^(https?:|data:|blob:|asset:|tauri:)/i.test(value);
}

async function resolveLocalImageDataUrl(path: string): Promise<string | null> {
  const value = String(path ?? "").trim();
  if (!value) return null;
  if (isDirectImageSrc(value)) return value;
  const cached = LOCAL_IMAGE_DATA_URL_CACHE.get(value);
  if (cached) return cached;
  const inFlight = LOCAL_IMAGE_DATA_URL_PENDING.get(value);
  if (inFlight) return inFlight;
  const task = readLocalImageDataUrl({ path: value })
    .then((data) => {
      const normalized = String(data ?? "").trim();
      if (!normalized) return null;
      LOCAL_IMAGE_DATA_URL_CACHE.set(value, normalized);
      return normalized;
    })
    .catch(() => null)
    .finally(() => {
      LOCAL_IMAGE_DATA_URL_PENDING.delete(value);
    });
  LOCAL_IMAGE_DATA_URL_PENDING.set(value, task);
  return task;
}

export function LocalImage({
  path,
  alt,
  fallback = null,
}: {
  path?: string | null;
  alt: string;
  fallback?: ReactNode;
}) {
  const normalizedPath = String(path ?? "").trim();
  const [src, setSrc] = useState<string | null>(() => {
    if (!normalizedPath) return null;
    if (isDirectImageSrc(normalizedPath)) return normalizedPath;
    return LOCAL_IMAGE_DATA_URL_CACHE.get(normalizedPath) ?? null;
  });

  useEffect(() => {
    let cancelled = false;
    if (!normalizedPath) {
      setSrc(null);
      return () => {
        cancelled = true;
      };
    }
    if (isDirectImageSrc(normalizedPath)) {
      setSrc(normalizedPath);
      return () => {
        cancelled = true;
      };
    }
    const cached = LOCAL_IMAGE_DATA_URL_CACHE.get(normalizedPath);
    if (cached) {
      setSrc(cached);
      return () => {
        cancelled = true;
      };
    }
    setSrc(null);
    void resolveLocalImageDataUrl(normalizedPath).then((resolved) => {
      if (!cancelled) setSrc(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [normalizedPath]);

  if (!src) return <>{fallback}</>;
  return <img src={src} alt={alt} loading="lazy" decoding="async" />;
}

export function RemoteImage({
  src,
  alt,
  fallback = null,
}: {
  src?: string | null;
  alt: string;
  fallback?: ReactNode;
}) {
  const normalized = String(src ?? "").trim();
  const [loadFailed, setLoadFailed] = useState(false);

  useEffect(() => {
    setLoadFailed(false);
  }, [normalized]);

  if (!normalized || loadFailed) return <>{fallback}</>;
  return (
    <img
      src={normalized}
      alt={alt}
      loading="lazy"
      decoding="async"
      onError={() => setLoadFailed(true)}
    />
  );
}
