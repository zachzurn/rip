package com.zachzurn.rip

/**
 * Thrown when rendering fails (e.g. empty document with no renderable content).
 */
class RipRenderException(message: String) : Exception(message)
