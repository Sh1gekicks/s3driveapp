// 入力の検証（03 §4.2、DLG-01／10）。フォームのスキーマと名前の検証。

import { z } from 'zod';
import { ja } from './i18n/ja';

const t = ja.connection;

export const ACCESS_KEY_ID = /^AKIA[A-Z0-9]{16}$/;
export const ROLE_ARN = /^arn:aws(-[a-z]+)?:iam::\d{12}:role\/[\w+=,.@/-]{1,512}$/;
export const BUCKET = /^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$/;

export const connectionSchema = z
  .object({
    credentialMode: z.enum(['new', 'existing']),
    credentialId: z.string(),
    accessKeyId: z.string().trim(),
    secretAccessKey: z.string(),
    roleArn: z.string().trim(),
    externalId: z.string().trim(),
    region: z.string().min(1, t.required),
    bucket: z.string().trim(),
  })
  .superRefine((v, ctx) => {
    if (v.credentialMode === 'new') {
      if (!ACCESS_KEY_ID.test(v.accessKeyId)) {
        ctx.addIssue({
          code: 'custom',
          path: ['accessKeyId'],
          message: v.accessKeyId ? t.invalidAccessKeyId : t.required,
        });
      }
      if (v.secretAccessKey.length !== 40) {
        ctx.addIssue({
          code: 'custom',
          path: ['secretAccessKey'],
          message: v.secretAccessKey ? t.invalidSecret : t.required,
        });
      }
    } else if (!v.credentialId) {
      ctx.addIssue({ code: 'custom', path: ['credentialId'], message: t.required });
    }
    if (v.roleArn && !ROLE_ARN.test(v.roleArn)) {
      ctx.addIssue({ code: 'custom', path: ['roleArn'], message: t.invalidRoleArn });
    }
    if (!BUCKET.test(v.bucket) || v.bucket.includes('..')) {
      ctx.addIssue({ code: 'custom', path: ['bucket'], message: v.bucket ? t.invalidBucket : t.required });
    }
  });

export type ConnectionFormValues = z.infer<typeof connectionSchema>;

export const credentialSchema = z.object({
  accessKeyId: z
    .string()
    .trim()
    .refine((v) => ACCESS_KEY_ID.test(v), { message: t.invalidAccessKeyId }),
  secretAccessKey: z.string().length(40, t.invalidSecret),
});

export type CredentialFormValues = z.infer<typeof credentialSchema>;

const encoder = new TextEncoder();

/** フォルダ・ファイルの名前の検証（DLG-01／10）。問題がなければ null。 */
export function validateName(name: string, prefix: string, siblings: Iterable<string> = []): string | null {
  const d = ja.dialog;
  const trimmed = name.trim();
  if (!trimmed) return d.name.empty;
  if (trimmed.includes('/')) return d.name.slash;
  if (trimmed === '.' || trimmed === '..') return d.name.dots;
  if (encoder.encode(`${prefix}${trimmed}/`).length > 1024) return d.name.tooLong;
  for (const s of siblings) if (s === trimmed) return d.newFolder.duplicate;
  return null;
}
