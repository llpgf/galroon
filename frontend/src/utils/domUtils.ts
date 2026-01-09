/**
 * DOM Utilities - Phase 18.1: Architectural Sanitization
 *
 * Centralized helpers for DOM interactions that require
 * non-standard HTML attributes.
 */

/**
 * Directory input attributes for folder selection
 *
 * Browsers require webkitdirectory (Chrome/Edge) or directory (Firefox)
 * attributes to enable folder selection in <input type="file">.
 *
 * TypeScript doesn't recognize these as valid HTML attributes,
 * so we use a type assertion in a centralized location.
 *
 * Usage:
 * <input type="file" {...directoryInputProps} />
 */
import type { InputHTMLAttributes } from 'react';

type DirectoryInputProps = InputHTMLAttributes<HTMLInputElement> & {
  webkitdirectory?: string;
  directory?: string;
};

export const directoryInputProps: DirectoryInputProps = {
  webkitdirectory: '',
  directory: '',
};
