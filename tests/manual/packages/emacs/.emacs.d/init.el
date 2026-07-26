;; ~/.emacs.d/init.el
(package-initialize)
(add-to-list 'package-archives '(“melpa” . “https://melpa.org/packages/”))

(require 'use-package)
(setq package-enable-at-startup nil)